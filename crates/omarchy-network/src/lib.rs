use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::unix::process::CommandExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SCHEMA_VERSION: u32 = 1;
const OUTPUT_LIMIT: usize = 64 * 1024;
const DEFAULT_AGENT_TIMEOUT_SECONDS: u64 = 300;
const PLUGIN_ID: &str = "omarchy-rs.network-inspector";
const PLUGIN_MANIFEST: &str =
    include_str!("../../../plugins/omarchy-rs.network-inspector/manifest.json");
const PLUGIN_PANEL: &str = include_str!("../../../plugins/omarchy-rs.network-inspector/Panel.qml");
const RUST_BADGE: &str = include_str!("../../../plugins/common/RustBadge.qml");

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Codex,
    Claude,
    Grok,
    Octos,
}

impl Agent {
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "codex" => Ok(Self::Codex),
            "claude" | "claude-code" => Ok(Self::Claude),
            "grok" => Ok(Self::Grok),
            "octos" | "octoscode" => Ok(Self::Octos),
            _ => Err(format!("unsupported Agent: {value}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Grok => "grok",
            Self::Octos => "octos",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NetworkLayout {
    pub home: PathBuf,
    pub state_root: PathBuf,
    pub config_root: PathBuf,
    pub proc_root: PathBuf,
    pub sys_root: PathBuf,
    pub resolver_path: PathBuf,
    pub path: Vec<PathBuf>,
    pub agents: BTreeMap<Agent, PathBuf>,
    pub hyprctl: PathBuf,
    pub getcap: PathBuf,
    pub terminal_launcher: PathBuf,
    pub cli_executable: PathBuf,
    pub agent_timeout: Duration,
}

impl NetworkLayout {
    pub fn from_environment() -> Result<Self, String> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is unset")?;
        let path = env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect();
        let configured = |name: &str, fallback: &str| {
            env::var_os(name)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(fallback))
        };
        let mut agents = BTreeMap::new();
        agents.insert(
            Agent::Codex,
            configured("OMARCHY_RS_NETWORK_CODEX", "codex"),
        );
        agents.insert(
            Agent::Claude,
            configured("OMARCHY_RS_NETWORK_CLAUDE", "claude"),
        );
        agents.insert(Agent::Grok, configured("OMARCHY_RS_NETWORK_GROK", "grok"));
        agents.insert(
            Agent::Octos,
            configured("OMARCHY_RS_NETWORK_OCTOS", "octos"),
        );
        Ok(Self {
            state_root: env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/state")),
            config_root: env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config")),
            proc_root: configured("OMARCHY_RS_NETWORK_PROC_ROOT", "/proc"),
            sys_root: configured("OMARCHY_RS_NETWORK_SYS_ROOT", "/sys"),
            resolver_path: configured("OMARCHY_RS_NETWORK_RESOLV_CONF", "/etc/resolv.conf"),
            hyprctl: configured("OMARCHY_RS_NETWORK_HYPRCTL", "hyprctl"),
            getcap: configured("OMARCHY_RS_NETWORK_GETCAP", "getcap"),
            terminal_launcher: configured(
                "OMARCHY_RS_NETWORK_TERMINAL_LAUNCHER",
                "omarchy-launch-floating-terminal-with-presentation",
            ),
            cli_executable: env::current_exe().map_err(|error| error.to_string())?,
            agent_timeout: Duration::from_secs(
                env::var("OMARCHY_RS_NETWORK_AGENT_TIMEOUT_SECONDS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(DEFAULT_AGENT_TIMEOUT_SECONDS)
                    .clamp(30, 900),
            ),
            home,
            path,
            agents,
        })
    }

    fn network_state(&self) -> PathBuf {
        self.state_root.join("omarchy-rs/network")
    }

    fn plan_path(&self, id: &str) -> Result<PathBuf, String> {
        safe_id(id)?;
        Ok(self
            .network_state()
            .join("plans")
            .join(format!("{id}.json")))
    }

    fn result_path(&self, id: &str) -> Result<PathBuf, String> {
        safe_id(id)?;
        Ok(self
            .network_state()
            .join("results")
            .join(format!("{id}.json")))
    }

    fn operation_path(&self) -> PathBuf {
        self.network_state().join("last-operation.json")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceStatus {
    pub name: String,
    pub kind: String,
    pub carrier: Option<bool>,
    pub oper_state: String,
    pub rx_bytes: Option<u64>,
    pub tx_bytes: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SniffnetStatus {
    pub installed: bool,
    pub running: bool,
    pub capture_ready: bool,
    pub capture_status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub schema_version: u32,
    pub default_route: bool,
    pub interface: Option<InterfaceStatus>,
    pub dns_configured: bool,
    pub sniffnet: SniffnetStatus,
    pub last_operation: Option<OperationStatus>,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationStatus {
    pub operation: String,
    pub outcome: String,
    pub code: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisPlan {
    pub schema_version: u32,
    pub id: String,
    pub confirmation_token: String,
    pub agent: Agent,
    pub snapshot_digest: String,
    pub snapshot: NetworkStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisResult {
    pub schema_version: u32,
    pub plan_id: String,
    pub agent: Agent,
    pub advice: String,
    #[serde(default)]
    pub advice_html: String,
    #[serde(default)]
    pub advice_markdown: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default = "cold_session_mode")]
    pub session_mode: String,
    pub commands_executed: u32,
}

fn cold_session_mode() -> String {
    "cold-fallback".into()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenReport {
    pub action: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTerminalReport {
    pub action: String,
    pub agent: Agent,
    pub plan_id: String,
}

pub fn status(layout: &NetworkLayout) -> NetworkStatus {
    let default_interface = default_interface(&layout.proc_root.join("net/route"));
    let interface = default_interface
        .as_deref()
        .map(|name| interface_status(layout, name));
    let default_route = default_interface.is_some();
    let dns_configured = resolver_configured(&layout.resolver_path);
    let sniffnet_executable = resolve_named(layout, Path::new("sniffnet"));
    let capture_ready = sniffnet_executable
        .as_deref()
        .is_some_and(|path| capture_ready(layout, path));
    let sniffnet = SniffnetStatus {
        installed: sniffnet_executable.is_some(),
        running: process_named(&layout.proc_root, "sniffnet"),
        capture_ready,
        capture_status: sniffnet_executable
            .as_deref()
            .map(|_| {
                if capture_ready {
                    "ready"
                } else {
                    "permission-required"
                }
            })
            .unwrap_or("not-installed")
            .into(),
    };
    let mut issues = Vec::new();
    let last_operation = read_operation(layout);
    if !default_route {
        issues.push("no-default-route".into());
    }
    if interface.as_ref().and_then(|value| value.carrier) == Some(false) {
        issues.push("link-carrier-down".into());
    }
    if !dns_configured {
        issues.push("dns-not-configured".into());
    }
    if !sniffnet.installed {
        issues.push("sniffnet-not-installed".into());
    } else if !sniffnet.capture_ready {
        issues.push("sniffnet-capture-permission-required".into());
    }
    if let Some(operation) = &last_operation
        && operation.outcome == "failed"
    {
        issues.push(operation.code.clone());
    }
    NetworkStatus {
        schema_version: SCHEMA_VERSION,
        default_route,
        interface,
        dns_configured,
        sniffnet,
        last_operation,
        issues,
    }
}

fn default_interface(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()?
        .lines()
        .skip(1)
        .find_map(|line| {
            let columns = line.split_whitespace().collect::<Vec<_>>();
            if columns.len() < 4 || columns[1] != "00000000" {
                return None;
            }
            let flags = u32::from_str_radix(columns[3], 16).ok()?;
            (flags & 0x2 != 0 && safe_interface(columns[0])).then(|| columns[0].to_owned())
        })
}

fn safe_interface(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn interface_status(layout: &NetworkLayout, name: &str) -> InterfaceStatus {
    let root = layout.sys_root.join("class/net").join(name);
    InterfaceStatus {
        name: name.into(),
        kind: if root.join("wireless").is_dir() {
            "wifi"
        } else {
            "ethernet"
        }
        .into(),
        carrier: read_trimmed(&root.join("carrier")).and_then(|value| match value.as_str() {
            "1" => Some(true),
            "0" => Some(false),
            _ => None,
        }),
        oper_state: read_trimmed(&root.join("operstate"))
            .filter(|value| matches!(value.as_str(), "up" | "down" | "dormant" | "unknown"))
            .unwrap_or_else(|| "unknown".into()),
        rx_bytes: read_u64(&root.join("statistics/rx_bytes")),
        tx_bytes: read_u64(&root.join("statistics/tx_bytes")),
    }
}

fn read_trimmed(path: &Path) -> Option<String> {
    let value = fs::read_to_string(path).ok()?;
    let value = value.trim();
    (!value.is_empty() && value.len() <= 128).then(|| value.to_owned())
}

fn read_u64(path: &Path) -> Option<u64> {
    read_trimmed(path)?.parse().ok()
}

fn resolver_configured(path: &Path) -> bool {
    fs::read_to_string(path).ok().is_some_and(|text| {
        text.lines().any(|line| {
            line.split_whitespace().next() == Some("nameserver")
                && line.split_whitespace().nth(1).is_some()
        })
    })
}

fn resolve_named(layout: &NetworkLayout, executable: &Path) -> Option<PathBuf> {
    if executable.components().count() > 1 || executable.is_absolute() {
        return is_executable(executable).then(|| executable.to_owned());
    }
    layout
        .path
        .iter()
        .map(|root| root.join(executable))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .ok()
        .is_some_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

fn process_ids_named(proc_root: &Path, name: &str) -> Vec<u32> {
    let mut ids = fs::read_dir(proc_root)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .filter(|value| value.bytes().all(|byte| byte.is_ascii_digit()))
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|_| read_trimmed(&entry.path().join("comm")).as_deref() == Some(name))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}

fn process_named(proc_root: &Path, name: &str) -> bool {
    !process_ids_named(proc_root, name).is_empty()
}

fn capture_ready(layout: &NetworkLayout, executable: &Path) -> bool {
    let Some(getcap) = resolve_named(layout, &layout.getcap) else {
        return false;
    };
    for _ in 0..2 {
        if let Ok(output) = Command::new(&getcap)
            .arg(executable)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
        {
            return text.contains("cap_net_raw") && text.contains("cap_net_admin");
        }
    }
    false
}

pub fn open_sniffnet(layout: &NetworkLayout) -> Result<OpenReport, String> {
    let executable = resolve_named(layout, Path::new("sniffnet")).ok_or_else(|| {
        record_operation(layout, "failed", "sniffnet-not-installed");
        "sniffnet-unavailable: install Sniffnet first"
    })?;
    let process_ids = process_ids_named(&layout.proc_root, "sniffnet");
    if !process_ids.is_empty() {
        let result = (|| {
            let hyprctl = resolve_named(layout, &layout.hyprctl)
                .ok_or("hyprctl-unavailable: cannot focus running Sniffnet")?;
            let clients = Command::new(&hyprctl)
                .args(["clients", "-j"])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .map_err(|_| "sniffnet-focus-failed: cannot query Hyprland clients")?;
            let values: serde_json::Value = serde_json::from_slice(&clients.stdout)
                .map_err(|_| "sniffnet-focus-failed: invalid Hyprland client response")?;
            let address = values
                .as_array()
                .and_then(|clients| {
                    clients.iter().find(|client| {
                        client
                            .get("pid")
                            .and_then(serde_json::Value::as_u64)
                            .is_some_and(|pid| process_ids.contains(&(pid as u32)))
                    })
                })
                .and_then(|client| client.get("address"))
                .and_then(serde_json::Value::as_str)
                .filter(|address| {
                    address.starts_with("0x")
                        && address[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                .ok_or("sniffnet-window-unavailable: running process has no Hyprland window")?;
            let selector = format!("hl.dsp.focus({{ window = \"address:{address}\" }})");
            let status = Command::new(hyprctl)
                .args(["dispatch", &selector])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|_| "sniffnet-focus-failed: hyprctl invocation failed")?;
            status
                .success()
                .then_some(())
                .ok_or("sniffnet-focus-failed: Hyprland rejected focus")
        })();
        if let Err(error) = result {
            let code = error.split(':').next().unwrap_or("sniffnet-focus-failed");
            record_operation(layout, "failed", code);
            return Err(error.into());
        }
        record_operation(layout, "succeeded", "sniffnet-focused");
        return Ok(OpenReport {
            action: "focused".into(),
        });
    }
    Command::new(executable)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            record_operation(layout, "failed", "sniffnet-launch-failed");
            "sniffnet-launch-failed: could not launch Sniffnet".to_owned()
        })?;
    record_operation(layout, "succeeded", "sniffnet-launched");
    Ok(OpenReport {
        action: "launched".into(),
    })
}

fn record_operation(layout: &NetworkLayout, outcome: &str, code: &str) {
    let _ = atomic_json(
        &layout.operation_path(),
        &OperationStatus {
            operation: "open-sniffnet".into(),
            outcome: outcome.into(),
            code: code.into(),
        },
    );
}

fn read_operation(layout: &NetworkLayout) -> Option<OperationStatus> {
    serde_json::from_slice(&fs::read(layout.operation_path()).ok()?).ok()
}

pub fn create_plan(layout: &NetworkLayout, agent: Agent) -> Result<DiagnosisPlan, String> {
    if agent == Agent::Octos {
        return Err("octos-unavailable: no supported interactive terminal interface".into());
    }
    let snapshot = status(layout);
    let snapshot_digest = stable_snapshot_digest(&snapshot)?;
    let nonce = now_nanos();
    let id = digest(format!("{snapshot_digest}:{}:{nonce}", agent.name()).as_bytes());
    let confirmation_token = digest(format!("confirm:{id}:{nonce}").as_bytes())[..12].to_owned();
    let plan = DiagnosisPlan {
        schema_version: SCHEMA_VERSION,
        id,
        confirmation_token,
        agent,
        snapshot_digest,
        snapshot,
    };
    atomic_json(&layout.plan_path(&plan.id)?, &plan)?;
    Ok(plan)
}

pub fn launch_agent_terminal(
    layout: &NetworkLayout,
    agent: Agent,
) -> Result<AgentTerminalReport, String> {
    let launcher = resolve_named(layout, &layout.terminal_launcher)
        .ok_or("terminal-launcher-unavailable: Omarchy terminal launcher was not found")?;
    let plan = create_plan(layout, agent)?;
    let command = [
        shell_quote(layout.cli_executable.to_str().ok_or("invalid CLI path")?),
        "network".into(),
        "agent-session".into(),
        "--plan".into(),
        plan.id.clone(),
        "--confirm".into(),
        plan.confirmation_token.clone(),
    ]
    .join(" ");
    Command::new(launcher)
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("terminal-launch-failed: {error}"))?;
    Ok(AgentTerminalReport {
        action: "launched".into(),
        agent,
        plan_id: plan.id,
    })
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn confirmed_plan(
    layout: &NetworkLayout,
    plan_id: &str,
    confirmation: &str,
) -> Result<DiagnosisPlan, String> {
    let plan: DiagnosisPlan = serde_json::from_slice(
        &fs::read(layout.plan_path(plan_id)?).map_err(|_| "diagnosis plan not found")?,
    )
    .map_err(|_| "invalid diagnosis plan")?;
    if plan.schema_version != SCHEMA_VERSION || plan.id != plan_id {
        return Err("invalid diagnosis plan identity".into());
    }
    if plan.confirmation_token != confirmation {
        return Err("diagnosis confirmation token mismatch".into());
    }
    if stable_snapshot_digest(&status(layout))? != plan.snapshot_digest {
        return Err("network snapshot changed; create a new diagnosis plan".into());
    }
    Ok(plan)
}

fn interactive_agent_invocation(
    layout: &NetworkLayout,
    plan: &DiagnosisPlan,
) -> Result<(PathBuf, Vec<String>, PathBuf), String> {
    let configured = layout.agents.get(&plan.agent).ok_or("Agent unavailable")?;
    let executable = resolve_named(layout, configured)
        .ok_or_else(|| format!("{}-unavailable", plan.agent.name()))?;
    let snapshot = serde_json::to_string_pretty(&plan.snapshot).map_err(|e| e.to_string())?;
    let prompt = format!(
        "Help me diagnose this local network issue interactively. Start by interpreting the content-free snapshot below, explain the strongest evidence, and ask me for any additional safe checks you need. Do not make changes without my explicit approval.\n\n<network_snapshot>\n{snapshot}\n</network_snapshot>"
    );
    let args = match plan.agent {
        Agent::Codex | Agent::Claude => vec![prompt],
        Agent::Grok => vec!["--disable-web-search".into(), prompt],
        Agent::Octos => {
            return Err("octos-unavailable: no supported interactive terminal interface".into());
        }
    };
    Ok((executable, args, layout.home.clone()))
}

pub fn start_agent_session(
    layout: &NetworkLayout,
    plan_id: &str,
    confirmation: &str,
) -> Result<String, String> {
    let plan = confirmed_plan(layout, plan_id, confirmation)?;
    let (executable, args, working_directory) = interactive_agent_invocation(layout, &plan)?;
    let error = Command::new(executable)
        .args(args)
        .current_dir(working_directory)
        .exec();
    Err(format!("agent-session-failed: {error}"))
}

pub fn diagnose(
    layout: &NetworkLayout,
    plan_id: &str,
    confirmation: &str,
) -> Result<DiagnosisResult, String> {
    let plan: DiagnosisPlan = serde_json::from_slice(
        &fs::read(layout.plan_path(plan_id)?).map_err(|_| "diagnosis plan not found")?,
    )
    .map_err(|_| "invalid diagnosis plan")?;
    if plan.schema_version != SCHEMA_VERSION || plan.id != plan_id {
        return Err("invalid diagnosis plan identity".into());
    }
    if plan.confirmation_token != confirmation {
        return Err("diagnosis confirmation token mismatch".into());
    }
    let current = status(layout);
    if stable_snapshot_digest(&current)? != plan.snapshot_digest {
        return Err("network snapshot changed; create a new diagnosis plan".into());
    }
    let invocation = agent_invocation(layout, &plan)?;
    let run = run_agent(invocation, layout.agent_timeout)?;
    let advice = run.advice;
    if advice.trim().is_empty() {
        return Err("Agent returned empty advice".into());
    }
    let result = DiagnosisResult {
        schema_version: SCHEMA_VERSION,
        plan_id: plan.id.clone(),
        agent: plan.agent,
        advice_html: render_markdown(&advice),
        advice_markdown: safe_markdown(&advice),
        advice,
        session_id: run.session_id,
        session_mode: "new-session".into(),
        commands_executed: 0,
    };
    atomic_json(&layout.result_path(&plan.id)?, &result)?;
    Ok(result)
}

pub fn follow_up(
    layout: &NetworkLayout,
    plan_id: &str,
    confirmation: &str,
    question: &str,
) -> Result<DiagnosisResult, String> {
    let question = question.trim();
    if question.is_empty() || question.len() > 4096 {
        return Err("follow-up question must contain 1–4096 bytes".into());
    }
    let plan: DiagnosisPlan = serde_json::from_slice(
        &fs::read(layout.plan_path(plan_id)?).map_err(|_| "diagnosis plan not found")?,
    )
    .map_err(|_| "invalid diagnosis plan")?;
    if plan.confirmation_token != confirmation
        || stable_snapshot_digest(&status(layout))? != plan.snapshot_digest
    {
        return Err("follow-up confirmation or snapshot mismatch".into());
    }
    let previous = load_result(layout, plan_id)?;
    let (invocation, session_mode) = if let Some(session_id) = previous.session_id.as_deref() {
        let prompt = format!(
            "Continue this network diagnosis without tools, browsing, commands, or secrets. Answer directly in Markdown.\n\n<follow_up>\n{question}\n</follow_up>\n"
        )
        .into_bytes();
        (
            agent_resume_invocation(layout, &plan, session_id, prompt)?,
            "warm-session",
        )
    } else {
        let prompt = format!(
            "Continue the prior content-free network diagnosis. Do not browse, call tools, execute commands, or request secrets. Answer the follow-up directly in Markdown.\n\n<prior_advice>\n{}\n</prior_advice>\n<follow_up>\n{}\n</follow_up>\n",
            previous.advice, question
        )
        .into_bytes();
        let mut invocation = agent_invocation(layout, &plan)?;
        invocation.stdin = prompt;
        (invocation, "cold-fallback")
    };
    let run = run_agent(invocation, layout.agent_timeout)?;
    let advice = run.advice;
    if advice.trim().is_empty() {
        return Err("Agent returned empty advice".into());
    }
    let result = DiagnosisResult {
        schema_version: SCHEMA_VERSION,
        plan_id: plan.id.clone(),
        agent: plan.agent,
        advice_html: render_markdown(&advice),
        advice_markdown: safe_markdown(&advice),
        advice,
        session_id: run.session_id.or(previous.session_id),
        session_mode: session_mode.into(),
        commands_executed: 0,
    };
    atomic_json(&layout.result_path(&plan.id)?, &result)?;
    Ok(result)
}

fn render_markdown(markdown: &str) -> String {
    let parser = pulldown_cmark::Parser::new_ext(markdown, pulldown_cmark::Options::all());
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    ammonia::Builder::default()
        .tags(
            [
                "p",
                "br",
                "pre",
                "code",
                "strong",
                "em",
                "del",
                "blockquote",
                "ul",
                "ol",
                "li",
                "h1",
                "h2",
                "h3",
                "h4",
                "h5",
                "h6",
                "hr",
                "table",
                "thead",
                "tbody",
                "tr",
                "th",
                "td",
            ]
            .into_iter()
            .collect(),
        )
        .generic_attributes(Default::default())
        .clean(&html)
        .to_string()
}

fn safe_markdown(markdown: &str) -> String {
    markdown
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace("![", "[")
        .replace("]( ", "] (")
        .replace("](http", "] (http")
}

pub fn load_result(layout: &NetworkLayout, id: &str) -> Result<DiagnosisResult, String> {
    serde_json::from_slice(
        &fs::read(layout.result_path(id)?).map_err(|_| "diagnosis result not found")?,
    )
    .map_err(|_| "invalid diagnosis result".into())
}

fn stable_snapshot_digest(snapshot: &NetworkStatus) -> Result<String, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Stable<'a> {
        schema_version: u32,
        default_route: bool,
        interface_name: Option<&'a str>,
        interface_kind: Option<&'a str>,
        carrier: Option<bool>,
        oper_state: Option<&'a str>,
        dns_configured: bool,
        sniffnet_installed: bool,
        sniffnet_capture_ready: bool,
        issues: &'a [String],
    }
    let interface = snapshot.interface.as_ref();
    let stable = Stable {
        schema_version: snapshot.schema_version,
        default_route: snapshot.default_route,
        interface_name: interface.map(|value| value.name.as_str()),
        interface_kind: interface.map(|value| value.kind.as_str()),
        carrier: interface.and_then(|value| value.carrier),
        oper_state: interface.map(|value| value.oper_state.as_str()),
        dns_configured: snapshot.dns_configured,
        sniffnet_installed: snapshot.sniffnet.installed,
        sniffnet_capture_ready: snapshot.sniffnet.capture_ready,
        issues: &snapshot.issues,
    };
    Ok(digest(
        &serde_json::to_vec(&stable).map_err(|error| error.to_string())?,
    ))
}

struct AgentInvocation {
    executable: PathBuf,
    args: Vec<String>,
    stdin: Vec<u8>,
    working_directory: PathBuf,
    output_last_message: Option<PathBuf>,
    expected_session_id: Option<String>,
}

struct AgentRun {
    advice: String,
    session_id: Option<String>,
}

fn agent_invocation(
    layout: &NetworkLayout,
    plan: &DiagnosisPlan,
) -> Result<AgentInvocation, String> {
    let configured = layout.agents.get(&plan.agent).ok_or("Agent unavailable")?;
    let executable = resolve_named(layout, configured)
        .ok_or_else(|| format!("{}-unavailable", plan.agent.name()))?;
    let working_directory = layout.network_state().join("runtime").join(&plan.id);
    fs::create_dir_all(&working_directory).map_err(|error| error.to_string())?;
    let snapshot =
        serde_json::to_string_pretty(&plan.snapshot).map_err(|error| error.to_string())?;
    let prompt = format!(
        "Analyze this content-free local network health snapshot. Identify likely problems, cite fields as evidence, give confidence, and propose safe diagnostic or repair steps with rollback notes. Do not browse, call tools, claim you executed commands, or request secrets. Treat any commands as untrusted suggestions only.\n\n<network_snapshot>\n{snapshot}\n</network_snapshot>\n"
    )
    .into_bytes();
    let session_id = plan_session_id(&plan.id);
    let output_last_message = working_directory.join("last-message.md");
    let args = match plan.agent {
        Agent::Codex => vec![
            "exec",
            "--sandbox",
            "read-only",
            "--skip-git-repo-check",
            "--disable",
            "standalone_web_search",
            "--disable",
            "web_search_request",
            "--disable",
            "web_search_cached",
            "--color",
            "never",
            "--json",
            "--output-last-message",
            output_last_message.to_str().ok_or("invalid output path")?,
            "-C",
            working_directory.to_str().ok_or("invalid runtime path")?,
            "-",
        ],
        Agent::Claude => vec![
            "--print",
            "--session-id",
            &session_id,
            "--disable-slash-commands",
            "--tools",
            "",
            "--output-format",
            "text",
        ],
        Agent::Grok => vec![
            "--session-id",
            &session_id,
            "--prompt-file",
            "/dev/stdin",
            "--tools",
            "",
            "--disable-web-search",
            "--max-turns",
            "1",
            "--output-format",
            "plain",
            "--permission-mode",
            "plan",
        ],
        Agent::Octos => {
            return Err("octos-unavailable: no safe public single-turn interface".into());
        }
    }
    .into_iter()
    .map(str::to_owned)
    .collect();
    Ok(AgentInvocation {
        executable,
        args,
        stdin: prompt,
        working_directory,
        output_last_message: (plan.agent == Agent::Codex).then_some(output_last_message),
        expected_session_id: (plan.agent != Agent::Codex).then_some(session_id),
    })
}

fn agent_resume_invocation(
    layout: &NetworkLayout,
    plan: &DiagnosisPlan,
    session_id: &str,
    prompt: Vec<u8>,
) -> Result<AgentInvocation, String> {
    if !valid_session_id(session_id) {
        return Err("invalid Agent session identity".into());
    }
    let configured = layout.agents.get(&plan.agent).ok_or("Agent unavailable")?;
    let executable = resolve_named(layout, configured)
        .ok_or_else(|| format!("{}-unavailable", plan.agent.name()))?;
    let working_directory = layout.network_state().join("runtime").join(&plan.id);
    fs::create_dir_all(&working_directory).map_err(|error| error.to_string())?;
    let output_last_message = working_directory.join("last-message.md");
    let args = match plan.agent {
        Agent::Codex => vec![
            "exec",
            "resume",
            "--skip-git-repo-check",
            "--disable",
            "standalone_web_search",
            "--disable",
            "web_search_request",
            "--disable",
            "web_search_cached",
            "--json",
            "--output-last-message",
            output_last_message.to_str().ok_or("invalid output path")?,
            session_id,
            "-",
        ],
        Agent::Claude => vec![
            "--print",
            "--resume",
            session_id,
            "--disable-slash-commands",
            "--tools",
            "",
            "--output-format",
            "text",
        ],
        Agent::Grok => vec![
            "--resume",
            session_id,
            "--prompt-file",
            "/dev/stdin",
            "--tools",
            "",
            "--disable-web-search",
            "--max-turns",
            "1",
            "--output-format",
            "plain",
            "--permission-mode",
            "plan",
        ],
        Agent::Octos => {
            return Err("octos-unavailable: no safe public single-turn interface".into());
        }
    }
    .into_iter()
    .map(str::to_owned)
    .collect();
    Ok(AgentInvocation {
        executable,
        args,
        stdin: prompt,
        working_directory,
        output_last_message: (plan.agent == Agent::Codex).then_some(output_last_message),
        expected_session_id: Some(session_id.to_owned()),
    })
}

fn plan_session_id(plan_id: &str) -> String {
    format!(
        "{}-{}-{}-{}-{}",
        &plan_id[0..8],
        &plan_id[8..12],
        &plan_id[12..16],
        &plan_id[16..20],
        &plan_id[20..32]
    )
}

fn valid_session_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn run_agent(invocation: AgentInvocation, timeout: Duration) -> Result<AgentRun, String> {
    let output_last_message = invocation.output_last_message.clone();
    let expected_session_id = invocation.expected_session_id.clone();
    let mut child = Command::new(&invocation.executable)
        .args(&invocation.args)
        .current_dir(&invocation.working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Agent start failed: {error}"))?;
    child
        .stdin
        .take()
        .ok_or("Agent stdin unavailable")?
        .write_all(&invocation.stdin)
        .map_err(|error| error.to_string())?;
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = child.stdout.take().ok_or("Agent stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("Agent stderr unavailable")?;
    let out_overflow = overflow.clone();
    let err_overflow = overflow.clone();
    let out = thread::spawn(move || read_bounded(stdout, OUTPUT_LIMIT, out_overflow));
    let err = thread::spawn(move || read_bounded(stderr, OUTPUT_LIMIT, err_overflow));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Agent diagnosis timed out".into());
        }
        thread::sleep(Duration::from_millis(20));
    };
    let stdout = out.join().map_err(|_| "Agent stdout reader failed")?;
    let stderr = err.join().map_err(|_| "Agent stderr reader failed")?;
    if overflow.load(Ordering::Relaxed) {
        return Err("Agent diagnosis output exceeded 65536 bytes".into());
    }
    if !status.success() {
        return Err(format!(
            "Agent diagnosis failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    let stdout = String::from_utf8(stdout).map_err(|_| "Agent diagnosis output is not UTF-8")?;
    let advice = if let Some(path) = output_last_message {
        fs::read_to_string(path).map_err(|_| "Agent final message unavailable")?
    } else {
        stdout.clone()
    };
    let session_id = expected_session_id.or_else(|| codex_session_id(&stdout));
    Ok(AgentRun { advice, session_id })
}

fn codex_session_id(events: &str) -> Option<String> {
    events.lines().find_map(|line| {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        let kind = value.get("type")?.as_str()?;
        if !matches!(kind, "thread.started" | "session.started") {
            return None;
        }
        ["thread_id", "session_id"]
            .into_iter()
            .find_map(|key| value.get(key).and_then(serde_json::Value::as_str))
            .filter(|id| valid_session_id(id))
            .map(str::to_owned)
    })
}

fn read_bounded<R: Read>(mut reader: R, limit: usize, overflow: Arc<AtomicBool>) -> Vec<u8> {
    let mut result = Vec::new();
    let mut total = 0_usize;
    let mut buffer = [0_u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                total = total.saturating_add(count);
                if result.len() < limit {
                    result.extend_from_slice(&buffer[..count.min(limit - result.len())]);
                }
                if total > limit {
                    overflow.store(true, Ordering::Relaxed);
                }
            }
        }
    }
    result
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginOwner {
    schema_version: u32,
    plugin_id: String,
    files: Vec<PluginFile>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PluginFile {
    name: String,
    sha256: String,
}

pub fn install_plugin(layout: &NetworkLayout) -> Result<PathBuf, String> {
    if resolve_named(layout, Path::new("sniffnet")).is_none() {
        return Err("sniffnet-unavailable: install Sniffnet before enabling the plugin".into());
    }
    let destination = layout.config_root.join("omarchy/plugins").join(PLUGIN_ID);
    if destination.exists() {
        verify_plugin(&destination)?;
    } else {
        fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    }
    let files = [
        ("manifest.json", PLUGIN_MANIFEST),
        ("Panel.qml", PLUGIN_PANEL),
        ("RustBadge.qml", RUST_BADGE),
    ];
    for (name, content) in files {
        atomic_bytes(&destination.join(name), content.as_bytes())?;
    }
    atomic_json(
        &destination.join(".omarchy-rs-owner.json"),
        &PluginOwner {
            schema_version: SCHEMA_VERSION,
            plugin_id: PLUGIN_ID.into(),
            files: files
                .iter()
                .map(|(name, content)| PluginFile {
                    name: (*name).into(),
                    sha256: digest(content.as_bytes()),
                })
                .collect(),
        },
    )?;
    Ok(destination)
}

pub fn uninstall_plugin(layout: &NetworkLayout) -> Result<PathBuf, String> {
    let destination = layout.config_root.join("omarchy/plugins").join(PLUGIN_ID);
    let owner = verify_plugin(&destination)?;
    for file in owner.files {
        fs::remove_file(destination.join(file.name)).map_err(|error| error.to_string())?;
    }
    fs::remove_file(destination.join(".omarchy-rs-owner.json")).map_err(|e| e.to_string())?;
    fs::remove_dir(&destination).map_err(|_| "plugin contains unowned files")?;
    Ok(destination)
}

fn verify_plugin(destination: &Path) -> Result<PluginOwner, String> {
    let owner: PluginOwner = serde_json::from_slice(
        &fs::read(destination.join(".omarchy-rs-owner.json"))
            .map_err(|_| "refusing foreign plugin")?,
    )
    .map_err(|_| "invalid plugin ownership marker")?;
    if owner.plugin_id != PLUGIN_ID || owner.schema_version != SCHEMA_VERSION {
        return Err("mismatched plugin ownership marker".into());
    }
    let mut names = owner
        .files
        .iter()
        .map(|file| file.name.as_str())
        .collect::<BTreeSet<_>>();
    names.insert(".omarchy-rs-owner.json");
    for entry in fs::read_dir(destination).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !names.contains(entry.file_name().to_string_lossy().as_ref()) {
            return Err("plugin contains foreign files".into());
        }
    }
    for file in &owner.files {
        let bytes = fs::read(destination.join(&file.name)).map_err(|_| "plugin file missing")?;
        if digest(&bytes) != file.sha256 {
            return Err("plugin file modified".into());
        }
    }
    Ok(owner)
}

fn safe_id(id: &str) -> Result<(), String> {
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid network artifact id".into());
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    atomic_bytes(
        path,
        &serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?,
    )
}

fn atomic_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("artifact path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .ok_or("artifact has no name")?
            .to_string_lossy(),
        std::process::id()
    ));
    if temp.exists() {
        fs::remove_file(&temp).map_err(|error| error.to_string())?;
    }
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|error| error.to_string())
}

pub fn execute_cli(args: &[String]) -> Result<String, String> {
    let layout = NetworkLayout::from_environment()?;
    match args.first().map(String::as_str) {
        Some("status")
            if matches!(args, [command] if command == "status")
                || matches!(args, [command, flag] if command == "status" && flag == "--json") =>
        {
            json(&status(&layout))
        }
        Some("open")
            if matches!(args, [command] if command == "open")
                || matches!(args, [command, flag] if command == "open" && flag == "--json") =>
        {
            json(&open_sniffnet(&layout)?)
        }
        Some("plan") => {
            let flags = flags(&args[1..])?;
            json(&create_plan(
                &layout,
                Agent::parse(one(&flags, "--agent")?)?,
            )?)
        }
        Some("agent-terminal") => {
            let flags = flags(&args[1..])?;
            json(&launch_agent_terminal(
                &layout,
                Agent::parse(one(&flags, "--agent")?)?,
            )?)
        }
        Some("agent-session") => {
            let flags = flags(&args[1..])?;
            start_agent_session(&layout, one(&flags, "--plan")?, one(&flags, "--confirm")?)
        }
        Some("diagnose") => {
            let flags = flags(&args[1..])?;
            json(&diagnose(
                &layout,
                one(&flags, "--plan")?,
                one(&flags, "--confirm")?,
            )?)
        }
        Some("follow-up") => {
            let flags = flags(&args[1..])?;
            let mut question = String::new();
            std::io::stdin()
                .take(4097)
                .read_to_string(&mut question)
                .map_err(|error| error.to_string())?;
            json(&follow_up(
                &layout,
                one(&flags, "--plan")?,
                one(&flags, "--confirm")?,
                &question,
            )?)
        }
        Some("result") => {
            let flags = flags(&args[1..])?;
            json(&load_result(&layout, one(&flags, "--plan")?)?)
        }
        Some("install-plugin") if args.len() == 1 => {
            Ok(format!("installed {}", install_plugin(&layout)?.display()))
        }
        Some("uninstall-plugin") if args.len() == 1 => Ok(format!(
            "uninstalled {}",
            uninstall_plugin(&layout)?.display()
        )),
        _ => Err(network_usage()),
    }
}

fn json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|error| error.to_string())
}

fn flags(args: &[String]) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut output = BTreeMap::<String, Vec<String>>::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--json" {
            index += 1;
            continue;
        }
        if !args[index].starts_with("--") || index + 1 >= args.len() {
            return Err(network_usage());
        }
        output
            .entry(args[index].clone())
            .or_default()
            .push(args[index + 1].clone());
        index += 2;
    }
    Ok(output)
}

fn one<'a>(flags: &'a BTreeMap<String, Vec<String>>, name: &str) -> Result<&'a str, String> {
    match flags.get(name).map(Vec::as_slice) {
        Some([value]) => Ok(value),
        _ => Err(format!("missing or repeated {name}\n{}", network_usage())),
    }
}

fn network_usage() -> String {
    "usage: omarchy-rs network <status --json|open --json|agent-terminal --agent codex|claude|grok --json|install-plugin|uninstall-plugin>".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tempfile::TempDir;

    struct Fixture {
        _temp: TempDir,
        layout: NetworkLayout,
        bin: PathBuf,
        receipt: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let home = temp.path().join("home");
            let proc_root = temp.path().join("proc");
            let sys_root = temp.path().join("sys");
            let bin = temp.path().join("bin");
            fs::create_dir_all(proc_root.join("net")).unwrap();
            fs::create_dir_all(sys_root.join("class/net/wlan0/statistics")).unwrap();
            fs::create_dir_all(sys_root.join("class/net/wlan0/wireless")).unwrap();
            fs::create_dir_all(&bin).unwrap();
            fs::write(
                proc_root.join("net/route"),
                "Iface Destination Gateway Flags RefCnt Use Metric Mask MTU Window IRTT\n\
                 wlan0 00000000 0101A8C0 0003 0 0 600 00000000 0 0 0\n",
            )
            .unwrap();
            let iface = sys_root.join("class/net/wlan0");
            fs::write(iface.join("carrier"), "1\n").unwrap();
            fs::write(iface.join("operstate"), "up\n").unwrap();
            fs::write(iface.join("statistics/rx_bytes"), "1234\n").unwrap();
            fs::write(iface.join("statistics/tx_bytes"), "5678\n").unwrap();
            let resolver = temp.path().join("resolv.conf");
            fs::write(&resolver, "nameserver 192.0.2.53\n").unwrap();
            let receipt = temp.path().join("receipt");
            let sniffnet = bin.join("sniffnet");
            executable(
                &sniffnet,
                &format!("#!/bin/sh\nprintf 'launch\\n' >> '{}'\n", receipt.display()),
            );
            let getcap = bin.join("getcap");
            executable(
                &getcap,
                "#!/bin/sh\nprintf '%s cap_net_raw,cap_net_admin=ep\\n' \"$1\"\n",
            );
            let hyprctl = bin.join("hyprctl");
            executable(
                &hyprctl,
                &format!(
                    "#!/bin/sh\nif [ \"$1\" = clients ]; then printf '[{{\"pid\":4242,\"address\":\"0xabc\"}}]\\n'; else printf 'focus:%s\\n' \"$*\" >> '{}'; fi\n",
                    receipt.display()
                ),
            );
            let terminal_launcher = bin.join("terminal-launcher");
            executable(
                &terminal_launcher,
                &format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$*\" > '{}.terminal'\n",
                    receipt.display()
                ),
            );
            let mut agents = BTreeMap::new();
            for (agent, name) in [
                (Agent::Codex, "codex"),
                (Agent::Claude, "claude"),
                (Agent::Grok, "grok"),
                (Agent::Octos, "octos"),
            ] {
                let path = bin.join(name);
                let response = if agent == Agent::Codex {
                    format!(
                        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}.argv'\ncat >> '{}.stdin'\nout=''\nprev=''\nfor arg in \"$@\"; do [ \"$prev\" = '--output-last-message' ] && out=\"$arg\"; prev=\"$arg\"; done\nprintf 'Evidence: local snapshot.\\nSuggestion: sudo imaginary-command (not executed).\\n' > \"$out\"\nprintf '{{\"type\":\"thread.started\",\"thread_id\":\"11111111-2222-3333-4444-555555555555\"}}\\n'\n",
                        receipt.display(),
                        receipt.display()
                    )
                } else {
                    format!(
                        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}.argv'\ncat >> '{}.stdin'\nprintf 'Evidence: local snapshot.\\nSuggestion: sudo imaginary-command (not executed).\\n'\n",
                        receipt.display(),
                        receipt.display()
                    )
                };
                executable(&path, &response);
                agents.insert(agent, path);
            }
            Self {
                layout: NetworkLayout {
                    home,
                    state_root: temp.path().join("state"),
                    config_root: temp.path().join("config"),
                    proc_root,
                    sys_root,
                    resolver_path: resolver,
                    path: vec![bin.clone()],
                    agents,
                    hyprctl,
                    getcap,
                    terminal_launcher,
                    cli_executable: PathBuf::from("/safe/omarchy-rs"),
                    agent_timeout: Duration::from_secs(2),
                },
                _temp: temp,
                bin,
                receipt,
            }
        }

        fn set_running(&self, running: bool) {
            let process = self.layout.proc_root.join("4242");
            if running {
                fs::create_dir_all(&process).unwrap();
                fs::write(process.join("comm"), "sniffnet\n").unwrap();
            } else if process.exists() {
                fs::remove_dir_all(process).unwrap();
            }
        }
    }

    fn executable(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn wait_for(path: &Path) {
        for _ in 0..100 {
            if path.exists() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for {}", path.display());
    }

    #[test]
    fn network_status_uses_synthetic_content_free_roots() {
        let fixture = Fixture::new();
        let report = status(&fixture.layout);
        assert!(report.default_route);
        assert!(report.dns_configured);
        assert_eq!(report.interface.as_ref().unwrap().name, "wlan0");
        assert_eq!(report.interface.as_ref().unwrap().kind, "wifi");
        assert_eq!(report.interface.as_ref().unwrap().rx_bytes, Some(1234));
        assert!(report.sniffnet.installed);
        assert!(report.sniffnet.capture_ready);
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("192.0.2.53"));
        assert!(!json.contains("0101A8C0"));
    }

    #[test]
    fn network_status_excludes_sensitive_and_malformed_values() {
        let fixture = Fixture::new();
        let markers = [
            "SECRET_SSID",
            "https://private.example/path",
            "token-secret",
            "203.0.113.99",
            "PAYLOAD_SENTINEL",
        ];
        fs::write(
            fixture.layout.resolver_path.clone(),
            format!(
                "# {} {} {} {}\nnameserver 203.0.113.99\n",
                markers[0], markers[1], markers[2], markers[4]
            ),
        )
        .unwrap();
        fs::write(
            fixture
                .layout
                .sys_root
                .join("class/net/wlan0/statistics/rx_bytes"),
            "not-a-number\n",
        )
        .unwrap();
        let report = status(&fixture.layout);
        assert_eq!(report.interface.as_ref().unwrap().rx_bytes, None);
        let plan = create_plan(&fixture.layout, Agent::Codex).unwrap();
        let bytes = serde_json::to_string(&(report, plan)).unwrap();
        for marker in markers {
            assert!(!bytes.contains(marker));
        }
    }

    #[test]
    fn network_open_launches_or_focuses_exactly_once() {
        let fixture = Fixture::new();
        fixture.set_running(false);
        assert_eq!(open_sniffnet(&fixture.layout).unwrap().action, "launched");
        wait_for(&fixture.receipt);
        assert_eq!(fs::read_to_string(&fixture.receipt).unwrap(), "launch\n");
        fs::write(&fixture.receipt, "").unwrap();
        fixture.set_running(true);
        assert_eq!(open_sniffnet(&fixture.layout).unwrap().action, "focused");
        let receipt = fs::read_to_string(&fixture.receipt).unwrap();
        assert_eq!(receipt.lines().count(), 1);
        assert_eq!(
            receipt,
            "focus:dispatch hl.dsp.focus({ window = \"address:0xabc\" })\n"
        );
    }

    #[test]
    fn network_focus_matches_hyprland_client_by_pid() {
        let fixture = Fixture::new();
        fixture.set_running(true);
        assert_eq!(open_sniffnet(&fixture.layout).unwrap().action, "focused");
        assert_eq!(
            fs::read_to_string(&fixture.receipt).unwrap(),
            "focus:dispatch hl.dsp.focus({ window = \"address:0xabc\" })\n"
        );
    }

    #[test]
    fn network_focus_failure_enters_diagnosis_snapshot() {
        let fixture = Fixture::new();
        fixture.set_running(true);
        executable(&fixture.layout.hyprctl, "#!/bin/sh\nprintf '[]\\n'\n");
        assert!(
            open_sniffnet(&fixture.layout)
                .unwrap_err()
                .contains("sniffnet-window-unavailable")
        );
        let plan = create_plan(&fixture.layout, Agent::Codex).unwrap();
        assert!(
            plan.snapshot
                .issues
                .contains(&"sniffnet-window-unavailable".to_owned())
        );
        let json = serde_json::to_string(&plan).unwrap();
        assert!(!json.contains("Hyprland client"));
    }

    #[test]
    fn network_agent_terminal_launch_is_direct_and_contextual() {
        for agent in [Agent::Codex, Agent::Claude, Agent::Grok] {
            let fixture = Fixture::new();
            let report = launch_agent_terminal(&fixture.layout, agent).unwrap();
            assert_eq!(report.action, "launched");
            let terminal_receipt = PathBuf::from(format!("{}.terminal", fixture.receipt.display()));
            wait_for(&terminal_receipt);
            let command = fs::read_to_string(terminal_receipt).unwrap();
            assert!(command.starts_with("'/safe/omarchy-rs' network agent-session --plan "));
            assert!(!command.contains("network_snapshot"));

            let plan: DiagnosisPlan = serde_json::from_slice(
                &fs::read(fixture.layout.plan_path(&report.plan_id).unwrap()).unwrap(),
            )
            .unwrap();
            let (_, args, working_directory) =
                interactive_agent_invocation(&fixture.layout, &plan).unwrap();
            let joined = args.join(" ");
            assert!(joined.contains("<network_snapshot>"));
            assert!(joined.contains("content-free snapshot"));
            assert_eq!(working_directory, fixture.layout.home);
            for forbidden in ["192.0.2.53", "0101A8C0", "cookie", "SECRET_SSID"] {
                assert!(!joined.contains(forbidden));
            }
        }
    }

    #[test]
    fn network_agent_terminal_rejects_unsupported_or_missing_tools() {
        let mut fixture = Fixture::new();
        assert!(launch_agent_terminal(&fixture.layout, Agent::Octos).is_err());
        fixture.layout.terminal_launcher = fixture.bin.join("missing-terminal");
        assert!(
            launch_agent_terminal(&fixture.layout, Agent::Codex)
                .unwrap_err()
                .contains("terminal-launcher-unavailable")
        );
        assert!(!fixture.receipt.exists());
    }

    #[test]
    fn network_agent_timeout_is_configurable_and_reported() {
        let mut fixture = Fixture::new();
        fixture.layout.agent_timeout = Duration::from_millis(40);
        executable(
            fixture.layout.agents.get(&Agent::Codex).unwrap(),
            "#!/bin/sh\nsleep 2\nprintf 'too late\\n'\n",
        );
        let plan = create_plan(&fixture.layout, Agent::Codex).unwrap();
        assert_eq!(
            diagnose(&fixture.layout, &plan.id, &plan.confirmation_token).unwrap_err(),
            "Agent diagnosis timed out"
        );
        assert!(!fixture.layout.result_path(&plan.id).unwrap().exists());
    }

    #[test]
    fn network_markdown_and_follow_up_are_sanitized() {
        let fixture = Fixture::new();
        let rendered = render_markdown(
            "# Heading\n\n**safe** <script>bad()</script> [link](https://example.invalid) ![pixel](https://example.invalid/pixel.png)",
        );
        assert!(rendered.contains("<h1>Heading</h1>"));
        assert!(rendered.contains("<strong>safe</strong>"));
        for forbidden in ["script", "href", "img", "https://"] {
            assert!(!rendered.contains(forbidden));
        }
        let safe = safe_markdown(
            "# Heading\n\n**safe** <script>bad()</script> [link](https://example.invalid) ![pixel](https://example.invalid/pixel.png)",
        );
        assert!(safe.contains("# Heading"));
        assert!(safe.contains("**safe**"));
        assert!(!safe.contains("<script>"));
        assert!(!safe.contains("!["));
        assert!(!safe.contains("](https://"));

        let plan = create_plan(&fixture.layout, Agent::Codex).unwrap();
        diagnose(&fixture.layout, &plan.id, &plan.confirmation_token).unwrap();
        let result = follow_up(
            &fixture.layout,
            &plan.id,
            &plan.confirmation_token,
            "Explain the focus issue",
        )
        .unwrap();
        assert!(!result.advice_html.is_empty());
        let stdin = fs::read_to_string(format!("{}.stdin", fixture.receipt.display())).unwrap();
        assert!(stdin.contains("<follow_up>\nExplain the focus issue\n</follow_up>"));
        let argv = fs::read_to_string(format!("{}.argv", fixture.receipt.display())).unwrap();
        assert!(!argv.contains("Explain the focus issue"));
    }

    #[test]
    fn network_follow_up_resumes_agent_session() {
        for agent in [Agent::Codex, Agent::Claude, Agent::Grok] {
            let fixture = Fixture::new();
            let plan = create_plan(&fixture.layout, agent).unwrap();
            let first = diagnose(&fixture.layout, &plan.id, &plan.confirmation_token).unwrap();
            let session_id = first.session_id.clone().expect("session identity");
            fs::write(format!("{}.argv", fixture.receipt.display()), "").unwrap();
            fs::write(format!("{}.stdin", fixture.receipt.display()), "").unwrap();
            let follow = follow_up(
                &fixture.layout,
                &plan.id,
                &plan.confirmation_token,
                "What should I verify next?",
            )
            .unwrap();
            assert_eq!(follow.session_mode, "warm-session");
            assert_eq!(follow.session_id.as_deref(), Some(session_id.as_str()));
            let argv = fs::read_to_string(format!("{}.argv", fixture.receipt.display())).unwrap();
            match agent {
                Agent::Codex => assert!(argv.contains("exec resume")),
                Agent::Claude | Agent::Grok => assert!(argv.contains("--resume")),
                Agent::Octos => unreachable!(),
            }
            assert!(argv.contains(&session_id));
            assert!(!argv.contains("What should I verify next?"));
            let stdin = fs::read_to_string(format!("{}.stdin", fixture.receipt.display())).unwrap();
            assert!(stdin.contains("What should I verify next?"));
            assert!(!stdin.contains("<prior_advice>"));
        }
    }

    #[test]
    fn network_open_missing_sniffnet_fails_without_side_effects() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.bin.join("sniffnet")).unwrap();
        assert!(
            open_sniffnet(&fixture.layout)
                .unwrap_err()
                .contains("sniffnet-unavailable")
        );
        assert!(!fixture.receipt.exists());
    }

    #[test]
    fn network_plugin_requires_sniffnet() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.bin.join("sniffnet")).unwrap();
        assert!(
            install_plugin(&fixture.layout)
                .unwrap_err()
                .contains("install Sniffnet before enabling")
        );
        assert!(
            !fixture
                .layout
                .config_root
                .join("omarchy/plugins")
                .join(PLUGIN_ID)
                .exists()
        );
    }

    #[test]
    fn network_agent_adapters_are_direct_and_bounded() {
        for agent in [Agent::Codex, Agent::Claude, Agent::Grok] {
            let fixture = Fixture::new();
            let plan = create_plan(&fixture.layout, agent).unwrap();
            let result = diagnose(&fixture.layout, &plan.id, &plan.confirmation_token).unwrap();
            assert_eq!(result.commands_executed, 0);
            assert!(result.advice.contains("not executed"));
            let argv = fs::read_to_string(format!("{}.argv", fixture.receipt.display())).unwrap();
            let stdin = fs::read_to_string(format!("{}.stdin", fixture.receipt.display())).unwrap();
            assert!(!argv.contains("sh -c"));
            assert!(stdin.contains("content-free local network health snapshot"));
            for forbidden in ["192.0.2.53", "0101A8C0", "cookie", "SSID"] {
                assert!(!stdin.contains(forbidden));
            }
        }
    }

    #[test]
    fn network_diagnosis_rejects_wrong_or_stale_plan() {
        let fixture = Fixture::new();
        let plan = create_plan(&fixture.layout, Agent::Codex).unwrap();
        assert!(diagnose(&fixture.layout, &plan.id, "wrong").is_err());
        assert!(!fixture.receipt.with_extension("argv").exists());
        fs::write(
            fixture.layout.sys_root.join("class/net/wlan0/carrier"),
            "0\n",
        )
        .unwrap();
        assert!(
            diagnose(&fixture.layout, &plan.id, &plan.confirmation_token)
                .unwrap_err()
                .contains("snapshot changed")
        );
        assert!(!fixture.layout.result_path(&plan.id).unwrap().exists());
    }

    #[test]
    fn network_diagnosis_rejects_octos_and_oversized_output() {
        let fixture = Fixture::new();
        assert!(
            create_plan(&fixture.layout, Agent::Octos)
                .unwrap_err()
                .contains("octos-unavailable")
        );
        let codex = fixture.layout.agents.get(&Agent::Codex).unwrap();
        executable(codex, "#!/bin/sh\nyes x | head -c 70000\n");
        let plan = create_plan(&fixture.layout, Agent::Codex).unwrap();
        assert!(
            diagnose(&fixture.layout, &plan.id, &plan.confirmation_token)
                .unwrap_err()
                .contains("exceeded")
        );
        assert!(!fixture.layout.result_path(&plan.id).unwrap().exists());
    }

    #[test]
    fn network_plugin_install_is_user_owned() {
        let fixture = Fixture::new();
        let destination = install_plugin(&fixture.layout).unwrap();
        for name in [
            "manifest.json",
            "Panel.qml",
            "RustBadge.qml",
            ".omarchy-rs-owner.json",
        ] {
            assert!(destination.join(name).is_file());
        }
        uninstall_plugin(&fixture.layout).unwrap();
        assert!(!destination.exists());
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("foreign"), "mine").unwrap();
        assert!(install_plugin(&fixture.layout).is_err());
    }

    #[test]
    fn network_plugin_uses_guarded_json_commands() {
        for command in ["status", "open", "agent-terminal"] {
            assert!(PLUGIN_PANEL.contains(&format!("\"network\", \"{command}\"")));
        }
        for removed in ["\"plan\"", "\"diagnose\"", "\"follow-up\""] {
            assert!(!PLUGIN_PANEL.contains(removed));
        }
        assert!(PLUGIN_PANEL.contains("textFormat: Text.PlainText"));
        assert!(!PLUGIN_PANEL.contains("TextEdit.MarkdownText"));
        assert!(PLUGIN_PANEL.contains("property bool sniffnetRunning"));
        assert!(PLUGIN_PANEL.contains("root.sniffnetRunning = true"));
        assert!(PLUGIN_PANEL.contains("text: root.sniffnetRunning ? \"Focus Sniffnet\""));
        assert!(PLUGIN_PANEL.contains("PanelHero"));
        assert!(PLUGIN_PANEL.contains("trailingControl: Component"));
        assert!(PLUGIN_PANEL.contains("RustBadge"));
        assert!(PLUGIN_PANEL.contains("report.sniffnet.installed"));
        for forbidden in ["bash", "sh -c", "sudo", "pkexec", "pcap"] {
            assert!(!PLUGIN_PANEL.to_ascii_lowercase().contains(forbidden));
        }
    }
}
