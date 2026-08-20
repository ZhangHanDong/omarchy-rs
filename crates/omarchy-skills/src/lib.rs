use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{BufRead, BufReader, Write},
    os::unix::fs::{MetadataExt, symlink},
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const SCHEMA_VERSION: u32 = 1;
const FRONTMATTER_LIMIT: u64 = 64 * 1024;
const PLUGIN_ID: &str = "omarchy-rs.skills";
const PLUGIN_MANIFEST: &str = include_str!("../../../plugins/omarchy-rs.skills/manifest.json");
const PLUGIN_PANEL: &str = include_str!("../../../plugins/omarchy-rs.skills/Panel.qml");
const RUST_BADGE: &str = include_str!("../../../plugins/common/RustBadge.qml");

#[derive(Clone, Debug)]
pub struct SkillsLayout {
    pub home: PathBuf,
    pub state_root: PathBuf,
    pub config_root: PathBuf,
    pub octos_executable: PathBuf,
    pub octos_profile: String,
}

impl SkillsLayout {
    pub fn from_environment() -> Result<Self, String> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is unset")?;
        Ok(Self {
            state_root: env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/state")),
            config_root: env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config")),
            octos_executable: env::var_os("OMARCHY_RS_OCTOS")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".octos/bin/octos")),
            octos_profile: env::var("OMARCHY_RS_OCTOS_PROFILE").unwrap_or_else(|_| "octos".into()),
            home,
        })
    }

    fn shared_root(&self) -> PathBuf {
        self.home.join(".agents/skills")
    }

    fn agent_root(&self, agent: Agent) -> Option<PathBuf> {
        match agent {
            Agent::Claude => Some(self.home.join(".claude/skills")),
            Agent::Codex => Some(self.home.join(".codex/skills")),
            Agent::Grok | Agent::Octoscode => None,
        }
    }

    fn plans_dir(&self) -> PathBuf {
        self.state_root.join("omarchy-rs/skills/plans")
    }

    fn receipts_path(&self) -> PathBuf {
        self.state_root.join("omarchy-rs/skills/receipts.json")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    Claude,
    Codex,
    Grok,
    Octoscode,
}

impl Agent {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "claude" | "claude-code" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "grok" => Ok(Self::Grok),
            "octos" | "octoscode" => Ok(Self::Octoscode),
            _ => Err(format!("unknown agent: {value}")),
        }
    }

    fn all() -> [Self; 4] {
        [Self::Claude, Self::Codex, Self::Grok, Self::Octoscode]
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentActivation {
    pub agent: Agent,
    pub state: String,
    pub destination: Option<PathBuf>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_class: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub healthy: bool,
    pub health_reason: Option<String>,
    pub activations: Vec<AgentActivation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsReport {
    pub schema_version: u32,
    pub shared_root: PathBuf,
    pub skills: Vec<SkillRecord>,
    pub agent_availability: BTreeMap<Agent, String>,
    pub total_unique_bytes: u64,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Sync,
    Cancel,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsPlan {
    pub schema_version: u32,
    pub id: String,
    pub confirmation_token: String,
    pub created_unix_ms: u64,
    pub operation: Operation,
    pub skill_name: String,
    pub source: PathBuf,
    pub source_identity: String,
    pub agents: Vec<Agent>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyItem {
    pub agent: Agent,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub schema_version: u32,
    pub plan_id: String,
    pub items: Vec<ApplyItem>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipts {
    schema_version: u32,
    links: Vec<LinkReceipt>,
    octos: Vec<OctosReceipt>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LinkReceipt {
    agent: Agent,
    skill_name: String,
    destination: PathBuf,
    source: PathBuf,
    source_identity: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OctosReceipt {
    skill_name: String,
    profile: String,
    source_identity: String,
}

pub fn scan(layout: &SkillsLayout) -> Result<SkillsReport, String> {
    let shared_root = layout.shared_root();
    let mut warnings = Vec::new();
    let mut records = Vec::new();
    let mut shared_names = BTreeSet::new();
    if shared_root.exists() {
        for entry in sorted_dirs(&shared_root, &mut warnings)? {
            let name = entry
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or("non-UTF-8 Skill name")?
                .to_string();
            shared_names.insert(name.clone());
            records.push(record_shared(layout, &entry, name)?);
        }
    }

    for agent in [Agent::Claude, Agent::Codex] {
        let root = layout.agent_root(agent).unwrap();
        if !root.exists() {
            continue;
        }
        for path in sorted_entries(&root, &mut warnings)? {
            let name = match path.file_name().and_then(|value| value.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };
            if shared_names.contains(&name) {
                continue;
            }
            records.push(record_native(agent, &path, name)?);
        }
    }

    for (class, root) in [
        ("grok-user", layout.home.join(".grok/skills")),
        ("grok-bundled", layout.home.join(".grok/bundled/skills")),
    ] {
        if !root.exists() {
            continue;
        }
        for path in sorted_dirs(&root, &mut warnings)? {
            let name = match path.file_name().and_then(|value| value.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };
            if shared_names.contains(&name) {
                continue;
            }
            records.push(record_grok(&path, name, class)?);
        }
    }

    records.sort_by(|left, right| left.name.cmp(&right.name).then(left.path.cmp(&right.path)));
    let mut availability = BTreeMap::new();
    availability.insert(Agent::Claude, "available".into());
    availability.insert(Agent::Codex, "available".into());
    availability.insert(Agent::Grok, "available".into());
    availability.insert(
        Agent::Octoscode,
        if layout.octos_executable.is_file() {
            "available".into()
        } else {
            "unavailable".into()
        },
    );
    Ok(SkillsReport {
        schema_version: SCHEMA_VERSION,
        shared_root,
        total_unique_bytes: records.iter().map(|record| record.bytes).sum(),
        skills: records,
        agent_availability: availability,
        warnings,
    })
}

fn record_shared(
    layout: &SkillsLayout,
    path: &Path,
    fallback: String,
) -> Result<SkillRecord, String> {
    let metadata = skill_metadata(path, &fallback)?;
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    let identity = source_identity(&canonical)?;
    let receipts = read_receipts(layout).unwrap_or_default();
    let mut activations = Vec::new();
    for agent in Agent::all() {
        let activation = match agent {
            Agent::Claude | Agent::Codex => {
                let destination = layout.agent_root(agent).unwrap().join(&metadata.name);
                let state = match fs::read_link(&destination) {
                    Ok(target) if resolve_link(&destination, &target) == canonical => "active",
                    Ok(_) => "conflict",
                    Err(_) if destination.exists() => "conflict",
                    Err(_) => "inactive",
                };
                AgentActivation {
                    agent,
                    state: state.into(),
                    destination: Some(destination),
                    detail: None,
                }
            }
            Agent::Grok => AgentActivation {
                agent,
                state: "active".into(),
                destination: Some(canonical.clone()),
                detail: Some("native-.agents-discovery".into()),
            },
            Agent::Octoscode => {
                let installed = receipts.octos.iter().any(|receipt| {
                    receipt.skill_name == metadata.name
                        && receipt.profile == layout.octos_profile
                        && receipt.source_identity == identity
                });
                AgentActivation {
                    agent,
                    state: if !layout.octos_executable.is_file() {
                        "unavailable"
                    } else if installed {
                        "managed"
                    } else {
                        "inactive"
                    }
                    .into(),
                    destination: None,
                    detail: Some(format!("profile={}", layout.octos_profile)),
                }
            }
        };
        activations.push(activation);
    }
    Ok(SkillRecord {
        id: identity,
        name: metadata.name,
        description: metadata.description,
        source_class: if path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            "shared-linked".into()
        } else {
            "shared-user".into()
        },
        path: canonical,
        bytes: metadata.bytes,
        healthy: metadata.reason.is_none(),
        health_reason: metadata.reason,
        activations,
    })
}

fn record_native(agent: Agent, path: &Path, fallback: String) -> Result<SkillRecord, String> {
    let metadata = skill_metadata(path, &fallback)?;
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    Ok(SkillRecord {
        id: source_identity(path)?,
        name: metadata.name,
        description: metadata.description,
        source_class: if path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            "system-linked"
        } else {
            "agent-native"
        }
        .into(),
        path: canonical,
        bytes: metadata.bytes,
        healthy: metadata.reason.is_none(),
        health_reason: metadata.reason,
        activations: vec![AgentActivation {
            agent,
            state: "active-read-only".into(),
            destination: Some(path.to_path_buf()),
            detail: None,
        }],
    })
}

fn record_grok(path: &Path, fallback: String, class: &str) -> Result<SkillRecord, String> {
    let metadata = skill_metadata(path, &fallback)?;
    Ok(SkillRecord {
        id: source_identity(path)?,
        name: metadata.name,
        description: metadata.description,
        source_class: class.into(),
        path: path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
        bytes: metadata.bytes,
        healthy: metadata.reason.is_none(),
        health_reason: metadata.reason,
        activations: vec![AgentActivation {
            agent: Agent::Grok,
            state: "active-read-only".into(),
            destination: Some(path.to_path_buf()),
            detail: None,
        }],
    })
}

struct ParsedSkill {
    name: String,
    description: Option<String>,
    bytes: u64,
    reason: Option<String>,
}

fn skill_metadata(path: &Path, fallback: &str) -> Result<ParsedSkill, String> {
    let file = path.join("SKILL.md");
    let bytes = directory_size(path)?;
    let handle = match fs::File::open(&file) {
        Ok(handle) => handle,
        Err(_) => {
            return Ok(ParsedSkill {
                name: fallback.into(),
                description: None,
                bytes,
                reason: Some("missing-SKILL.md".into()),
            });
        }
    };
    let mut reader = BufReader::new(handle);
    let mut line = String::new();
    let mut read = reader
        .read_line(&mut line)
        .map_err(|error| error.to_string())? as u64;
    if line.trim_end() != "---" {
        return Ok(ParsedSkill {
            name: fallback.into(),
            description: None,
            bytes,
            reason: Some("missing-frontmatter".into()),
        });
    }
    let mut name = None;
    let mut description = None;
    let mut closed = false;
    loop {
        line.clear();
        let count = reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        read += count as u64;
        if read > FRONTMATTER_LIMIT {
            return Ok(ParsedSkill {
                name: fallback.into(),
                description: None,
                bytes,
                reason: Some("frontmatter-over-64-kib".into()),
            });
        }
        let trimmed = line.trim();
        if trimmed == "---" {
            closed = true;
            break;
        }
        if let Some(value) = trimmed.strip_prefix("name:") {
            name = Some(unquote(value.trim()).to_string());
        } else if let Some(value) = trimmed.strip_prefix("description:") {
            description = Some(unquote(value.trim()).to_string());
        }
    }
    let parsed_name = name.filter(|value| valid_name(value));
    Ok(ParsedSkill {
        name: parsed_name.clone().unwrap_or_else(|| fallback.into()),
        description,
        bytes,
        reason: if !closed {
            Some("unterminated-frontmatter".into())
        } else if parsed_name.is_none() {
            Some("invalid-or-missing-name".into())
        } else {
            None
        },
    })
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn sorted_entries(root: &Path, warnings: &mut Vec<String>) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(error) => warnings.push(error.to_string()),
        }
    }
    paths.sort();
    Ok(paths)
}

fn sorted_dirs(root: &Path, warnings: &mut Vec<String>) -> Result<Vec<PathBuf>, String> {
    Ok(sorted_entries(root, warnings)?
        .into_iter()
        .filter(|path| {
            fs::symlink_metadata(path)
                .map(|metadata| metadata.is_dir() || metadata.file_type().is_symlink())
                .unwrap_or(false)
        })
        .collect())
}

fn directory_size(root: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        } else if metadata.is_dir() {
            for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
                stack.push(entry.map_err(|error| error.to_string())?.path());
            }
        }
    }
    Ok(total)
}

fn source_identity(path: &Path) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let skill = fs::metadata(path.join("SKILL.md")).ok();
    let value = format!(
        "{}:{}:{}:{}:{}:{}",
        path.canonicalize()
            .map_err(|error| error.to_string())?
            .display(),
        metadata.dev(),
        metadata.ino(),
        skill.as_ref().map_or(0, fs::Metadata::len),
        skill.as_ref().map_or(0, fs::Metadata::mtime),
        skill.as_ref().map_or(0, fs::Metadata::mtime_nsec)
    );
    Ok(format!("{:x}", Sha256::digest(value.as_bytes())))
}

fn resolve_link(link: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        target.to_path_buf()
    } else {
        link.parent().unwrap_or(Path::new("/")).join(target)
    }
    .canonicalize()
    .unwrap_or_default()
}

pub fn create_plan(
    layout: &SkillsLayout,
    skill_name: &str,
    operation: Operation,
    agents: &[Agent],
) -> Result<SkillsPlan, String> {
    if !valid_name(skill_name) || agents.is_empty() {
        return Err("plan requires a valid Skill name and at least one Agent".into());
    }
    let source = layout.shared_root().join(skill_name);
    if !source.is_dir() {
        return Err("shared Skill does not exist".into());
    }
    let canonical = source.canonicalize().map_err(|error| error.to_string())?;
    if !canonical.starts_with(
        layout
            .shared_root()
            .canonicalize()
            .map_err(|e| e.to_string())?,
    ) {
        return Err("shared Skill escapes the canonical root".into());
    }
    let identity = source_identity(&canonical)?;
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis() as u64;
    let mut normalized = agents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    normalized.sort();
    let seed = serde_json::to_vec(&(created, skill_name, operation, &normalized, &identity))
        .map_err(|error| error.to_string())?;
    let id = format!("{:x}", Sha256::digest(&seed));
    let token = format!("{:x}", Sha256::digest([seed, b"confirm".to_vec()].concat()));
    let plan = SkillsPlan {
        schema_version: SCHEMA_VERSION,
        id: id.clone(),
        confirmation_token: token,
        created_unix_ms: created,
        operation,
        skill_name: skill_name.into(),
        source: canonical,
        source_identity: identity,
        agents: normalized,
    };
    atomic_json(&layout.plans_dir().join(format!("{id}.json")), &plan)?;
    Ok(plan)
}

pub fn apply_plan(layout: &SkillsLayout, id: &str, token: &str) -> Result<ApplyReport, String> {
    if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid Skill plan id".into());
    }
    let path = layout.plans_dir().join(format!("{id}.json"));
    let plan: SkillsPlan =
        serde_json::from_slice(&fs::read(path).map_err(|_| "Skill plan not found")?)
            .map_err(|_| "invalid Skill plan")?;
    if plan.id != id || plan.confirmation_token != token {
        return Err("confirmation token does not match Skill plan".into());
    }
    if source_identity(&plan.source).ok().as_deref() != Some(plan.source_identity.as_str()) {
        return Ok(ApplyReport {
            schema_version: SCHEMA_VERSION,
            plan_id: id.into(),
            items: plan
                .agents
                .iter()
                .map(|agent| ApplyItem {
                    agent: *agent,
                    status: "skipped".into(),
                    reason: Some("source-identity-changed".into()),
                })
                .collect(),
        });
    }
    let mut receipts = read_receipts(layout).unwrap_or(Receipts {
        schema_version: SCHEMA_VERSION,
        ..Receipts::default()
    });
    let mut items = Vec::new();
    for agent in &plan.agents {
        let result = apply_agent(layout, &plan, *agent, &mut receipts);
        items.push(match result {
            Ok(()) => ApplyItem {
                agent: *agent,
                status: "applied".into(),
                reason: None,
            },
            Err(reason) => ApplyItem {
                agent: *agent,
                status: "skipped".into(),
                reason: Some(reason),
            },
        });
    }
    atomic_json(&layout.receipts_path(), &receipts)?;
    Ok(ApplyReport {
        schema_version: SCHEMA_VERSION,
        plan_id: id.into(),
        items,
    })
}

fn apply_agent(
    layout: &SkillsLayout,
    plan: &SkillsPlan,
    agent: Agent,
    receipts: &mut Receipts,
) -> Result<(), String> {
    match agent {
        Agent::Claude | Agent::Codex => apply_link(layout, plan, agent, receipts),
        Agent::Grok => Ok(()),
        Agent::Octoscode => apply_octos(layout, plan, receipts),
    }
}

fn apply_link(
    layout: &SkillsLayout,
    plan: &SkillsPlan,
    agent: Agent,
    receipts: &mut Receipts,
) -> Result<(), String> {
    let destination = layout.agent_root(agent).unwrap().join(&plan.skill_name);
    let existing = receipts
        .links
        .iter()
        .position(|receipt| receipt.agent == agent && receipt.skill_name == plan.skill_name);
    match plan.operation {
        Operation::Sync => {
            if destination.symlink_metadata().is_ok() {
                let owned = existing.and_then(|index| receipts.links.get(index));
                let matches = owned.is_some_and(|receipt| {
                    fs::read_link(&destination)
                        .ok()
                        .is_some_and(|target| resolve_link(&destination, &target) == plan.source)
                        && receipt.destination == destination
                        && receipt.source == plan.source
                });
                if !matches {
                    return Err("foreign-destination".into());
                }
                return Ok(());
            }
            fs::create_dir_all(destination.parent().unwrap()).map_err(|error| error.to_string())?;
            symlink(&plan.source, &destination).map_err(|error| error.to_string())?;
            receipts.links.push(LinkReceipt {
                agent,
                skill_name: plan.skill_name.clone(),
                destination,
                source: plan.source.clone(),
                source_identity: plan.source_identity.clone(),
            });
            Ok(())
        }
        Operation::Cancel => {
            let Some(index) = existing else {
                return Err("not-owned".into());
            };
            let receipt = &receipts.links[index];
            let matches = fs::read_link(&destination)
                .ok()
                .is_some_and(|target| resolve_link(&destination, &target) == receipt.source);
            if !matches {
                return Err("owned-link-changed".into());
            }
            fs::remove_file(&destination).map_err(|error| error.to_string())?;
            receipts.links.remove(index);
            Ok(())
        }
    }
}

fn apply_octos(
    layout: &SkillsLayout,
    plan: &SkillsPlan,
    receipts: &mut Receipts,
) -> Result<(), String> {
    if !layout.octos_executable.is_file() {
        return Err("octos-unavailable".into());
    }
    let mut command = Command::new(&layout.octos_executable);
    command.args(["skills", "--profile", &layout.octos_profile]);
    match plan.operation {
        Operation::Sync => {
            command.arg("install").arg(&plan.source).arg("--force");
        }
        Operation::Cancel => {
            if !receipts.octos.iter().any(|receipt| {
                receipt.skill_name == plan.skill_name && receipt.profile == layout.octos_profile
            }) {
                return Err("octos-install-not-owned".into());
            }
            command.arg("remove").arg(&plan.skill_name);
        }
    }
    let output = command
        .output()
        .map_err(|error| format!("octos-exec: {error}"))?;
    if !output.status.success() {
        return Err(format!("octos-exit-{}", output.status.code().unwrap_or(-1)));
    }
    match plan.operation {
        Operation::Sync => {
            receipts.octos.retain(|receipt| {
                !(receipt.skill_name == plan.skill_name && receipt.profile == layout.octos_profile)
            });
            receipts.octos.push(OctosReceipt {
                skill_name: plan.skill_name.clone(),
                profile: layout.octos_profile.clone(),
                source_identity: plan.source_identity.clone(),
            });
        }
        Operation::Cancel => receipts.octos.retain(|receipt| {
            !(receipt.skill_name == plan.skill_name && receipt.profile == layout.octos_profile)
        }),
    }
    Ok(())
}

fn read_receipts(layout: &SkillsLayout) -> Result<Receipts, String> {
    if !layout.receipts_path().exists() {
        return Ok(Receipts {
            schema_version: SCHEMA_VERSION,
            ..Receipts::default()
        });
    }
    serde_json::from_slice(&fs::read(layout.receipts_path()).map_err(|e| e.to_string())?)
        .map_err(|error| error.to_string())
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

pub fn install_plugin(layout: &SkillsLayout) -> Result<PathBuf, String> {
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
                    sha256: format!("{:x}", Sha256::digest(content.as_bytes())),
                })
                .collect(),
        },
    )?;
    Ok(destination)
}

pub fn uninstall_plugin(layout: &SkillsLayout) -> Result<PathBuf, String> {
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
        if format!("{:x}", Sha256::digest(bytes)) != file.sha256 {
            return Err("plugin file modified".into());
        }
    }
    Ok(owner)
}

fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    atomic_bytes(
        path,
        &serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?,
    )
}

fn atomic_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("state path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap().to_string_lossy(),
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
    let layout = SkillsLayout::from_environment()?;
    match args.first().map(String::as_str) {
        Some("scan") => {
            serde_json::to_string_pretty(&scan(&layout)?).map_err(|error| error.to_string())
        }
        Some("plan") => {
            let mut skill = None;
            let mut operation = Operation::Sync;
            let mut agents = Vec::new();
            let mut index = 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--skill" if index + 1 < args.len() => {
                        skill = Some(args[index + 1].clone());
                        index += 2;
                    }
                    "--agent" if index + 1 < args.len() => {
                        agents.push(Agent::parse(&args[index + 1])?);
                        index += 2;
                    }
                    "--operation" if index + 1 < args.len() => {
                        operation = match args[index + 1].as_str() {
                            "sync" => Operation::Sync,
                            "cancel" => Operation::Cancel,
                            _ => return Err(skills_usage()),
                        };
                        index += 2;
                    }
                    "--json" => index += 1,
                    _ => return Err(skills_usage()),
                }
            }
            if agents.is_empty() {
                agents.extend(Agent::all());
            }
            serde_json::to_string_pretty(&create_plan(
                &layout,
                skill.as_deref().ok_or_else(skills_usage)?,
                operation,
                &agents,
            )?)
            .map_err(|error| error.to_string())
        }
        Some("apply") => {
            let mut id = None;
            let mut token = None;
            let mut index = 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--plan" if index + 1 < args.len() => {
                        id = Some(args[index + 1].clone());
                        index += 2;
                    }
                    "--confirm" if index + 1 < args.len() => {
                        token = Some(args[index + 1].clone());
                        index += 2;
                    }
                    "--json" => index += 1,
                    _ => return Err(skills_usage()),
                }
            }
            serde_json::to_string_pretty(&apply_plan(
                &layout,
                id.as_deref().ok_or_else(skills_usage)?,
                token.as_deref().ok_or_else(skills_usage)?,
            )?)
            .map_err(|error| error.to_string())
        }
        Some("install-plugin") if args.len() == 1 => {
            Ok(format!("installed {}", install_plugin(&layout)?.display()))
        }
        Some("uninstall-plugin") if args.len() == 1 => Ok(format!(
            "uninstalled {}",
            uninstall_plugin(&layout)?.display()
        )),
        _ => Err(skills_usage()),
    }
}

fn skills_usage() -> String {
    "usage: omarchy-rs skills <scan --json|plan --skill NAME [--operation sync|cancel] [--agent AGENT]... --json|apply --plan ID --confirm TOKEN --json|install-plugin|uninstall-plugin>".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct Fixture {
        _temp: TempDir,
        layout: SkillsLayout,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let home = temp.path().join("home");
            for dir in [
                ".agents/skills",
                ".claude/skills",
                ".codex/skills",
                ".grok/skills",
            ] {
                fs::create_dir_all(home.join(dir)).unwrap();
            }
            Self {
                layout: SkillsLayout {
                    state_root: temp.path().join("state"),
                    config_root: temp.path().join("config"),
                    octos_executable: temp.path().join("missing-octos"),
                    octos_profile: "test-profile".into(),
                    home,
                },
                _temp: temp,
            }
        }

        fn skill(&self, name: &str, body: &str) -> PathBuf {
            let dir = self.layout.shared_root().join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: Test {name}\n---\n{body}"),
            )
            .unwrap();
            dir
        }
    }

    #[test]
    fn skills_inventory_normalizes_four_agents() {
        let fixture = Fixture::new();
        fixture.skill("demo", "private-body");
        let report = scan(&fixture.layout).unwrap();
        assert_eq!(report.skills.len(), 1);
        assert_eq!(report.skills[0].activations.len(), 4);
        assert_eq!(report.agent_availability.len(), 4);
        assert!(report.skills[0].healthy);
    }

    #[test]
    fn skills_inventory_groups_shared_duplicates() {
        let fixture = Fixture::new();
        let source = fixture.skill("demo", "body");
        symlink(&source, fixture.layout.home.join(".claude/skills/demo")).unwrap();
        symlink(&source, fixture.layout.home.join(".codex/skills/demo")).unwrap();
        let report = scan(&fixture.layout).unwrap();
        assert_eq!(report.skills.len(), 1);
        assert_eq!(
            report.skills[0]
                .activations
                .iter()
                .filter(|a| a.state == "active")
                .count(),
            3
        );
    }

    #[test]
    fn skills_inventory_excludes_bodies_and_private_state() {
        let fixture = Fixture::new();
        fixture.skill("demo", "PRIVATE_PROMPT_SENTINEL");
        fs::create_dir_all(fixture.layout.home.join(".claude/logs")).unwrap();
        fs::write(
            fixture.layout.home.join(".claude/logs/private"),
            "CREDENTIAL_SENTINEL",
        )
        .unwrap();
        let json = serde_json::to_string(&scan(&fixture.layout).unwrap()).unwrap();
        assert!(json.contains("Test demo"));
        assert!(!json.contains("PRIVATE_PROMPT_SENTINEL"));
        assert!(!json.contains("CREDENTIAL_SENTINEL"));
    }

    #[test]
    fn skills_inventory_rejects_oversized_or_malformed_frontmatter() {
        let fixture = Fixture::new();
        let oversized = fixture.layout.shared_root().join("oversized");
        fs::create_dir_all(&oversized).unwrap();
        fs::write(
            oversized.join("SKILL.md"),
            format!(
                "---\nname: oversized\ndescription: {}\n---\nbody",
                "x".repeat(70_000)
            ),
        )
        .unwrap();
        let malformed = fixture.layout.shared_root().join("malformed");
        fs::create_dir_all(&malformed).unwrap();
        fs::write(malformed.join("SKILL.md"), "name: malformed\nSECRET_BODY").unwrap();
        fs::create_dir_all(fixture.layout.home.join(".codex/skills/container-only")).unwrap();
        let report = scan(&fixture.layout).unwrap();
        assert_eq!(
            report.skills.iter().filter(|skill| !skill.healthy).count(),
            3
        );
    }

    #[test]
    fn skills_apply_creates_owned_claude_and_codex_links() {
        let fixture = Fixture::new();
        fixture.skill("demo", "body");
        let plan = create_plan(
            &fixture.layout,
            "demo",
            Operation::Sync,
            &[Agent::Claude, Agent::Codex],
        )
        .unwrap();
        let result = apply_plan(&fixture.layout, &plan.id, &plan.confirmation_token).unwrap();
        assert!(result.items.iter().all(|item| item.status == "applied"));
        assert!(fixture.layout.home.join(".claude/skills/demo").is_symlink());
        assert!(fixture.layout.home.join(".codex/skills/demo").is_symlink());
        assert_eq!(read_receipts(&fixture.layout).unwrap().links.len(), 2);
    }

    #[test]
    fn skills_apply_refuses_foreign_destination() {
        let fixture = Fixture::new();
        fixture.skill("demo", "body");
        let foreign = fixture.layout.home.join(".codex/skills/demo");
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("keep"), "foreign").unwrap();
        let plan = create_plan(&fixture.layout, "demo", Operation::Sync, &[Agent::Codex]).unwrap();
        let result = apply_plan(&fixture.layout, &plan.id, &plan.confirmation_token).unwrap();
        assert_eq!(
            result.items[0].reason.as_deref(),
            Some("foreign-destination")
        );
        assert_eq!(fs::read_to_string(foreign.join("keep")).unwrap(), "foreign");
    }

    #[test]
    fn skills_cancel_removes_only_owned_link() {
        let fixture = Fixture::new();
        fixture.skill("demo", "body");
        let sync = create_plan(&fixture.layout, "demo", Operation::Sync, &[Agent::Claude]).unwrap();
        apply_plan(&fixture.layout, &sync.id, &sync.confirmation_token).unwrap();
        let foreign = fixture.layout.home.join(".codex/skills/demo");
        symlink(fixture.layout.shared_root().join("demo"), &foreign).unwrap();
        let cancel = create_plan(
            &fixture.layout,
            "demo",
            Operation::Cancel,
            &[Agent::Claude, Agent::Codex],
        )
        .unwrap();
        let result = apply_plan(&fixture.layout, &cancel.id, &cancel.confirmation_token).unwrap();
        assert!(!fixture.layout.home.join(".claude/skills/demo").exists());
        assert!(foreign.is_symlink());
        assert_eq!(result.items[1].reason.as_deref(), Some("not-owned"));
    }

    #[test]
    fn skills_grok_adapter_uses_shared_discovery() {
        let fixture = Fixture::new();
        fixture.skill("demo", "body");
        let plan = create_plan(&fixture.layout, "demo", Operation::Sync, &[Agent::Grok]).unwrap();
        apply_plan(&fixture.layout, &plan.id, &plan.confirmation_token).unwrap();
        assert!(!fixture.layout.home.join(".grok/skills/demo").exists());
        assert_eq!(
            scan(&fixture.layout).unwrap().skills[0].activations[2].state,
            "active"
        );
    }

    #[test]
    fn skills_octos_adapter_invokes_native_profile_install() {
        let fixture = Fixture::new();
        fixture.skill("demo", "body");
        let argv = fixture._temp.path().join("argv");
        fs::write(
            &fixture.layout.octos_executable,
            format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\n", argv.display()),
        )
        .unwrap();
        let mut permissions = fs::metadata(&fixture.layout.octos_executable)
            .unwrap()
            .permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o700);
        fs::set_permissions(&fixture.layout.octos_executable, permissions).unwrap();
        let plan = create_plan(
            &fixture.layout,
            "demo",
            Operation::Sync,
            &[Agent::Octoscode],
        )
        .unwrap();
        let result = apply_plan(&fixture.layout, &plan.id, &plan.confirmation_token).unwrap();
        assert_eq!(result.items[0].status, "applied");
        let args = fs::read_to_string(argv).unwrap();
        assert!(args.contains("skills\n--profile\ntest-profile\ninstall\n"));
        assert!(args.contains("\n--force\n"));
    }

    #[test]
    fn skills_octos_adapter_reports_unavailable() {
        let fixture = Fixture::new();
        fixture.skill("demo", "body");
        let plan = create_plan(
            &fixture.layout,
            "demo",
            Operation::Sync,
            &[Agent::Octoscode],
        )
        .unwrap();
        let result = apply_plan(&fixture.layout, &plan.id, &plan.confirmation_token).unwrap();
        assert_eq!(result.items[0].reason.as_deref(), Some("octos-unavailable"));
        assert!(!fixture.layout.home.join(".octos").exists());
    }

    #[test]
    fn skills_apply_rejects_wrong_confirmation() {
        let fixture = Fixture::new();
        fixture.skill("demo", "body");
        let plan = create_plan(&fixture.layout, "demo", Operation::Sync, &[Agent::Codex]).unwrap();
        assert!(apply_plan(&fixture.layout, &plan.id, "wrong").is_err());
        assert!(!fixture.layout.home.join(".codex/skills/demo").exists());
    }

    #[test]
    fn skills_apply_rejects_changed_source() {
        let fixture = Fixture::new();
        let skill = fixture.skill("demo", "body");
        let plan = create_plan(&fixture.layout, "demo", Operation::Sync, &[Agent::Codex]).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: demo\n---\nchanged-longer",
        )
        .unwrap();
        let result = apply_plan(&fixture.layout, &plan.id, &plan.confirmation_token).unwrap();
        assert_eq!(
            result.items[0].reason.as_deref(),
            Some("source-identity-changed")
        );
    }

    #[test]
    fn skills_plugin_install_is_user_owned() {
        let fixture = Fixture::new();
        let destination = install_plugin(&fixture.layout).unwrap();
        assert!(destination.starts_with(&fixture.layout.config_root));
        assert_eq!(
            fs::read_to_string(destination.join("Panel.qml")).unwrap(),
            PLUGIN_PANEL
        );
        assert_eq!(
            fs::read_to_string(destination.join("RustBadge.qml")).unwrap(),
            RUST_BADGE
        );
        uninstall_plugin(&fixture.layout).unwrap();
        assert!(!destination.exists());
    }

    #[test]
    fn skills_plugin_uses_rust_json_commands() {
        for command in ["scan", "plan", "apply"] {
            assert!(PLUGIN_PANEL.contains(&format!("\"skills\", \"{command}\"")));
        }
        for forbidden in [
            "[\"bash\"",
            "[\"sh\"",
            "[\"sudo\"",
            "[\"pkexec\"",
            "[\"pacman\"",
        ] {
            assert!(!PLUGIN_PANEL.contains(forbidden));
        }
    }

    #[test]
    fn skills_panel_groups_agent_tabs_and_highlights_rust() {
        for agent in ["Claude", "Codex", "Grok", "Octos"] {
            assert!(PLUGIN_PANEL.contains(&format!("label: \"{agent}\"")));
        }
        assert!(PLUGIN_PANEL.contains("function agentSkills()"));
        assert!(PLUGIN_PANEL.contains("\"--agent\", selectedAgent"));
        assert!(PLUGIN_PANEL.contains("RustBadge"));
        assert!(PLUGIN_PANEL.contains("highlighted: true"));
        assert!(RUST_BADGE.contains("accent: \"#ff6a1a\""));
    }
}
