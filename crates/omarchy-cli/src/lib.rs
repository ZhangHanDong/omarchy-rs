use std::{
    env,
    ffi::OsStr,
    fs,
    io::Write,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

use crate::activation::{
    ActivationConfig, CLAUDE_UPSTREAM_SHA256, CODEX_UPSTREAM_SHA256, OCTOSCODE_UPSTREAM_SHA256,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) const EXECUTABLES: [&str; 6] = [
    "omarchy-rs",
    "omarchy-agent-usage-update",
    "omarchy-agent-usage-codex-shadow",
    "omarchy-agent-usage-claude-shadow",
    "omarchy-agent-usage-octoscode-shadow",
    "omarchy-agent-usage-grok",
];
const UPDATER: &str = "omarchy-agent-usage-update";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug)]
pub struct Layout {
    pub data_root: PathBuf,
    pub config_root: PathBuf,
    pub state_root: PathBuf,
    pub upstream_root: PathBuf,
    pub octoscode_upstream: PathBuf,
    pub expected_fingerprints: [String; 3],
}

impl Layout {
    pub fn from_environment() -> Result<Self, String> {
        let home = env::var_os("HOME").map(PathBuf::from);
        let data = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|path| path.join(".local/share")))
            .ok_or("HOME and XDG_DATA_HOME are unset")?;
        let config = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|path| path.join(".config")))
            .ok_or("HOME and XDG_CONFIG_HOME are unset")?;
        let state = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|path| path.join(".local/state")))
            .ok_or("HOME and XDG_STATE_HOME are unset")?;
        let upstream_root = env::var_os("OMARCHY_RS_UPSTREAM_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/usr/share/omarchy/bin"));
        let octoscode_upstream = env::var_os("OMARCHY_RS_OCTOSCODE_UPSTREAM")
            .map(PathBuf::from)
            .or_else(|| home.map(|path| path.join(".local/bin/omarchy-agent-usage-octoscode")))
            .ok_or("HOME and OMARCHY_RS_OCTOSCODE_UPSTREAM are unset")?;
        Ok(Self {
            data_root: data.join("omarchy-rs"),
            config_root: config.join("omarchy-rs"),
            state_root: state,
            upstream_root,
            octoscode_upstream,
            expected_fingerprints: [
                CODEX_UPSTREAM_SHA256.into(),
                CLAUDE_UPSTREAM_SHA256.into(),
                OCTOSCODE_UPSTREAM_SHA256.into(),
            ],
        })
    }

    fn libexec(&self) -> PathBuf {
        self.data_root.join("libexec")
    }
    fn bin(&self) -> PathBuf {
        self.data_root.join("bin")
    }
    fn manifest(&self) -> PathBuf {
        self.config_root.join("install.json")
    }
    fn activation(&self) -> PathBuf {
        self.config_root.join("activation.json")
    }
    fn shim(&self) -> PathBuf {
        self.bin().join(UPDATER)
    }
    fn installed_updater(&self) -> PathBuf {
        self.libexec().join(UPDATER)
    }
    fn cli_shim(&self) -> PathBuf {
        self.bin().join("omarchy-rs")
    }
    fn short_cli_shim(&self) -> PathBuf {
        self.bin().join("omrs")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Version,
    Doctor { json: bool },
    Install,
    ActivateAgentUsage,
    Status { json: bool },
    RollbackAgentUsage,
}

impl Command {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        match args {
            [flag] if flag == "--version" || flag == "-V" => Ok(Self::Version),
            [command] if command == "install" => Ok(Self::Install),
            [command] if command == "doctor" => Ok(Self::Doctor { json: false }),
            [command, flag] if command == "doctor" && flag == "--json" => {
                Ok(Self::Doctor { json: true })
            }
            [command] if command == "status" => Ok(Self::Status { json: false }),
            [command, flag] if command == "status" && flag == "--json" => {
                Ok(Self::Status { json: true })
            }
            [command, component] if command == "activate" && component == "agent-usage" => {
                Ok(Self::ActivateAgentUsage)
            }
            [command, component] if command == "rollback" && component == "agent-usage" => {
                Ok(Self::RollbackAgentUsage)
            }
            _ => Err("usage: omarchy-rs <--version|-V|doctor [--json]|install|activate agent-usage|status [--json]|rollback agent-usage|cleaner ...|skills ...|learn ...>".into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallManifest {
    schema_version: u32,
    libexec: PathBuf,
    files: Vec<InstalledFile>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledFile {
    name: String,
    sha256: String,
}

pub fn execute(
    command: Command,
    layout: &Layout,
    release_dir: &Path,
    path: &OsStr,
) -> Result<String, String> {
    match command {
        Command::Version => Ok(format!("omarchy-rs {VERSION}")),
        Command::Install => install(layout, release_dir),
        Command::ActivateAgentUsage => activate(layout, path),
        Command::RollbackAgentUsage => rollback(layout),
        Command::Status { json } | Command::Doctor { json } => {
            let report = status(layout, path);
            if json {
                serde_json::to_string_pretty(&report).map_err(|error| error.to_string())
            } else {
                Ok(render_status(&report))
            }
        }
    }
}

fn install(layout: &Layout, release_dir: &Path) -> Result<String, String> {
    let sources = EXECUTABLES
        .iter()
        .map(|name| (name, release_dir.join(name)))
        .collect::<Vec<_>>();
    if let Some((_, missing)) = sources.iter().find(|(_, path)| !path.is_file()) {
        return Err(format!(
            "required release executable is missing: {}",
            missing.display()
        ));
    }
    fs::create_dir_all(layout.libexec()).map_err(|error| error.to_string())?;
    fs::create_dir_all(&layout.config_root).map_err(|error| error.to_string())?;
    let mut files = Vec::new();
    for (name, source) in sources {
        let target = layout.libexec().join(name);
        atomic_copy(&source, &target)?;
        files.push(InstalledFile {
            name: (*name).into(),
            sha256: fingerprint(&target)?,
        });
    }
    fs::create_dir_all(layout.bin()).map_err(|error| error.to_string())?;
    let installed_cli = layout.libexec().join("omarchy-rs");
    ensure_owned_symlink(&layout.cli_shim(), &installed_cli)?;
    ensure_owned_symlink(&layout.short_cli_shim(), &installed_cli)?;
    let manifest = InstallManifest {
        schema_version: 1,
        libexec: layout.libexec(),
        files,
    };
    atomic_json(&layout.manifest(), &manifest)?;
    Ok(format!("installed {}", layout.libexec().display()))
}

fn activate(layout: &Layout, path: &OsStr) -> Result<String, String> {
    let manifest = load_manifest(layout)?;
    verify_install(layout, &manifest)?;
    if !overlay_precedes_upstream(path, &layout.bin(), &layout.upstream_root) {
        return Err(format!(
            "overlay bin does not precede upstream in PATH: {}",
            layout.bin().display()
        ));
    }
    let shim = layout.shim();
    let updater = layout.installed_updater();
    if fs::symlink_metadata(&shim).is_ok() {
        if fs::read_link(&shim).ok().as_deref() != Some(updater.as_path()) {
            return Err(format!(
                "refusing to overwrite foreign shim: {}",
                shim.display()
            ));
        }
    } else {
        fs::create_dir_all(layout.bin()).map_err(|error| error.to_string())?;
        symlink(&updater, &shim).map_err(|error| error.to_string())?;
    }
    if let Err(error) = atomic_json(&layout.activation(), &ActivationConfig::agent_usage()) {
        let _ = fs::remove_file(&shim);
        return Err(error);
    }
    Ok("agent-usage activated; restart the Omarchy shell if it predates this PATH".into())
}

fn rollback(layout: &Layout) -> Result<String, String> {
    let shim = layout.shim();
    if fs::symlink_metadata(&shim).is_ok() {
        if fs::read_link(&shim).ok().as_deref() != Some(layout.installed_updater().as_path()) {
            return Err(format!(
                "refusing to remove foreign shim: {}",
                shim.display()
            ));
        }
        fs::remove_file(&shim).map_err(|error| error.to_string())?;
    }
    if layout.activation().exists() {
        fs::remove_file(layout.activation()).map_err(|error| error.to_string())?;
    }
    Ok("agent-usage rolled back; installed release retained".into())
}

fn status(layout: &Layout, path: &OsStr) -> Value {
    let activation = fs::read(layout.activation())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ActivationConfig>(&bytes).ok());
    let providers = provider_status(layout, activation.as_ref());
    let resolved = resolve_command(path, UPDATER);
    json!({
        "schemaVersion": 1,
        "installed": load_manifest(layout).is_ok(),
        "activated": activation.as_ref().is_some_and(|config| config.component == "agent-usage"),
        "overlayPrecedesUpstream": overlay_precedes_upstream(path, &layout.bin(), &layout.upstream_root),
        "resolvedUpdater": resolved,
        "providers": providers,
    })
}

fn provider_status(layout: &Layout, activation: Option<&ActivationConfig>) -> Vec<Value> {
    let specs = [
        (
            "codex",
            layout.upstream_root.join("omarchy-agent-usage-codex"),
            layout.expected_fingerprints[0].as_str(),
        ),
        (
            "claude",
            layout.upstream_root.join("omarchy-agent-usage-claude"),
            layout.expected_fingerprints[1].as_str(),
        ),
        (
            "octoscode",
            layout.octoscode_upstream.clone(),
            layout.expected_fingerprints[2].as_str(),
        ),
    ];
    let mut providers = specs
        .into_iter()
        .map(|(name, upstream, expected)| {
            let current = fingerprint(&upstream).ok();
            json!({
                "id": name,
                "enabled": activation.is_some_and(|config| config.enables(name)),
                "backend": state_backend(layout, name),
                "upstream": upstream,
                "compatibility": if current.as_deref() == Some(expected) { "verified" } else { "unverified" },
            })
        })
        .collect::<Vec<_>>();
    providers.push(json!({
        "id": "grok",
        "enabled": activation.is_some_and(|config| config.enables("grok")),
        "backend": state_backend(layout, "grok"),
        "upstream": Value::Null,
        "compatibility": "native",
    }));
    providers
}

fn state_backend(layout: &Layout, name: &str) -> String {
    fs::read(
        layout
            .state_root
            .join(format!("omarchy/agents/usage/{name}.json")),
    )
    .ok()
    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    .and_then(|record| record["collectorBackend"].as_str().map(str::to_owned))
    .unwrap_or_else(|| "unknown".into())
}

fn render_status(report: &Value) -> String {
    let mut lines = vec![format!(
        "Agent Usage: {}",
        if report["activated"].as_bool() == Some(true) {
            "active"
        } else {
            "inactive"
        }
    )];
    lines.push(format!(
        "Updater: {}",
        report["resolvedUpdater"].as_str().unwrap_or("not found")
    ));
    if let Some(providers) = report["providers"].as_array() {
        for provider in providers {
            lines.push(format!(
                "{}: {} ({})",
                provider["id"].as_str().unwrap_or("unknown"),
                provider["backend"].as_str().unwrap_or("unknown"),
                provider["compatibility"].as_str().unwrap_or("unverified")
            ));
        }
    }
    lines.join("\n")
}

fn load_manifest(layout: &Layout) -> Result<InstallManifest, String> {
    fs::read(layout.manifest())
        .map_err(|_| "omarchy-rs is not installed".to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| error.to_string()))
}

fn verify_install(layout: &Layout, manifest: &InstallManifest) -> Result<(), String> {
    if manifest.schema_version != 1 || manifest.libexec != layout.libexec() {
        return Err("installation manifest is incompatible".into());
    }
    let mut names = manifest
        .files
        .iter()
        .map(|file| file.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    let mut required = EXECUTABLES.to_vec();
    required.sort_unstable();
    if names != required {
        return Err("installation manifest has an incomplete executable set".into());
    }
    for file in &manifest.files {
        let path = layout.libexec().join(&file.name);
        if fingerprint(&path).as_deref() != Ok(&file.sha256) {
            return Err(format!("installed executable drifted: {}", path.display()));
        }
    }
    Ok(())
}

fn overlay_precedes_upstream(path: &OsStr, overlay: &Path, upstream: &Path) -> bool {
    let entries = env::split_paths(path).collect::<Vec<_>>();
    let overlay_index = entries.iter().position(|entry| entry == overlay);
    let upstream_index = entries.iter().position(|entry| entry == upstream);
    matches!((overlay_index, upstream_index), (Some(left), Some(right)) if left < right)
}

fn resolve_command(path: &OsStr, name: &str) -> Option<PathBuf> {
    env::split_paths(path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn atomic_copy(source: &Path, target: &Path) -> Result<(), String> {
    let parent = target.parent().ok_or("target has no parent")?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        target
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("artifact"),
        std::process::id()
    ));
    fs::copy(source, &temporary).map_err(|error| error.to_string())?;
    fs::rename(&temporary, target).map_err(|error| error.to_string())
}

fn ensure_owned_symlink(link: &Path, target: &Path) -> Result<(), String> {
    if fs::symlink_metadata(link).is_ok() {
        if fs::read_link(link).ok().as_deref() == Some(target) {
            return Ok(());
        }
        return Err(format!(
            "refusing to overwrite foreign shim: {}",
            link.display()
        ));
    }
    symlink(target, link).map_err(|error| error.to_string())
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path.parent().ok_or("target has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(OsStr::to_str).unwrap_or("state"),
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(&serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| error.to_string())
}

fn fingerprint(path: &Path) -> Result<String, String> {
    fs::read(path)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    use crate::activation::PROVIDERS;
    use tempfile::TempDir;

    struct Fixture {
        root: TempDir,
        layout: Layout,
        release: PathBuf,
        path: OsString,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let release = root.path().join("release");
            let upstream = root.path().join("upstream");
            fs::create_dir_all(&release).unwrap();
            fs::create_dir_all(&upstream).unwrap();
            for name in EXECUTABLES {
                fs::write(release.join(name), name).unwrap();
            }
            for name in [
                UPDATER,
                "omarchy-agent-usage-codex",
                "omarchy-agent-usage-claude",
            ] {
                fs::write(upstream.join(name), name).unwrap();
            }
            let layout = Layout {
                data_root: root.path().join("data/omarchy-rs"),
                config_root: root.path().join("config/omarchy-rs"),
                state_root: root.path().join("state"),
                upstream_root: upstream.clone(),
                octoscode_upstream: root.path().join("upstream-octoscode"),
                expected_fingerprints: [String::new(), String::new(), String::new()],
            };
            fs::write(&layout.octoscode_upstream, "octoscode").unwrap();
            let mut layout = layout;
            layout.expected_fingerprints = [
                fingerprint(&layout.upstream_root.join("omarchy-agent-usage-codex")).unwrap(),
                fingerprint(&layout.upstream_root.join("omarchy-agent-usage-claude")).unwrap(),
                fingerprint(&layout.octoscode_upstream).unwrap(),
            ];
            let path = env::join_paths([layout.bin(), upstream]).unwrap();
            Self {
                root,
                layout,
                release,
                path,
            }
        }
        fn install(&self) {
            install(&self.layout, &self.release).unwrap();
        }
        fn activate(&self) {
            self.install();
            activate(&self.layout, &self.path).unwrap();
        }
    }

    #[test]
    fn version_flags_report_package_version() {
        let fixture = Fixture::new();
        for flag in ["--version", "-V"] {
            let command = Command::parse(&[flag.into()]).unwrap();
            assert_eq!(command, Command::Version);
            assert_eq!(
                execute(command, &fixture.layout, &fixture.release, &fixture.path).unwrap(),
                format!("omarchy-rs {}", env!("CARGO_PKG_VERSION"))
            );
        }
    }

    #[test]
    fn install_copies_release_siblings_and_manifest() {
        let fixture = Fixture::new();
        fixture.install();
        let manifest = load_manifest(&fixture.layout).unwrap();
        assert_eq!(manifest.files.len(), EXECUTABLES.len());
        assert!(
            manifest
                .files
                .iter()
                .all(|file| fixture.layout.libexec().join(&file.name).is_file())
        );
        assert!(fixture.layout.manifest().starts_with(fixture.root.path()));
        assert_eq!(
            fs::read_link(fixture.layout.cli_shim()).unwrap(),
            fixture.layout.libexec().join("omarchy-rs")
        );
    }

    #[test]
    fn install_creates_owned_omrs_alias() {
        let fixture = Fixture::new();
        fixture.install();
        let installed_cli = fixture.layout.libexec().join("omarchy-rs");
        assert_eq!(
            fs::read_link(fixture.layout.cli_shim()).unwrap(),
            installed_cli
        );
        assert_eq!(
            fs::read_link(fixture.layout.short_cli_shim()).unwrap(),
            fixture.layout.libexec().join("omarchy-rs")
        );
    }

    #[test]
    fn install_rejects_missing_release_sibling() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.release.join(EXECUTABLES[0])).unwrap();
        assert!(install(&fixture.layout, &fixture.release).is_err());
        assert!(!fixture.layout.manifest().exists());
    }

    #[test]
    fn activate_agent_usage_creates_owned_shim_and_config() {
        let fixture = Fixture::new();
        fixture.activate();
        assert_eq!(
            fs::read_link(fixture.layout.shim()).unwrap(),
            fixture.layout.installed_updater()
        );
        let config: ActivationConfig =
            serde_json::from_slice(&fs::read(fixture.layout.activation()).unwrap()).unwrap();
        assert!(
            PROVIDERS
                .into_iter()
                .all(|provider| config.enables(provider))
        );
    }

    #[test]
    fn activate_agent_usage_enables_grok() {
        let fixture = Fixture::new();
        fixture.activate();
        let config: ActivationConfig =
            serde_json::from_slice(&fs::read(fixture.layout.activation()).unwrap()).unwrap();
        assert!(config.enables("grok"));
        let report = status(&fixture.layout, &fixture.path);
        let grok = report["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|provider| provider["id"] == "grok")
            .unwrap();
        assert_eq!(grok["compatibility"], "native");
    }

    #[test]
    fn activate_refuses_foreign_shim() {
        let fixture = Fixture::new();
        fixture.install();
        fs::create_dir_all(fixture.layout.bin()).unwrap();
        fs::write(fixture.layout.shim(), "foreign").unwrap();
        assert!(activate(&fixture.layout, &fixture.path).is_err());
        assert_eq!(fs::read(fixture.layout.shim()).unwrap(), b"foreign");
        assert!(!fixture.layout.activation().exists());
    }

    #[test]
    fn activate_refuses_unsupported_precedence() {
        let fixture = Fixture::new();
        fixture.install();
        let path = env::join_paths([&fixture.layout.upstream_root, &fixture.layout.bin()]).unwrap();
        assert!(activate(&fixture.layout, &path).is_err());
        assert!(!fixture.layout.shim().exists());
    }

    #[test]
    fn status_json_reports_backend_and_drift() {
        let fixture = Fixture::new();
        fixture.activate();
        let usage = fixture.layout.state_root.join("omarchy/agents/usage");
        fs::create_dir_all(&usage).unwrap();
        fs::write(
            usage.join("codex.json"),
            br#"{"collectorBackend":"python"}"#,
        )
        .unwrap();
        fs::write(
            fixture
                .layout
                .upstream_root
                .join("omarchy-agent-usage-codex"),
            b"changed",
        )
        .unwrap();
        let report = status(&fixture.layout, &fixture.path);
        assert_eq!(report["activated"], true);
        assert_eq!(report["providers"][0]["compatibility"], "unverified");
        assert_eq!(report["providers"][0]["backend"], "python");
        assert_eq!(report["providers"][1]["compatibility"], "verified");
        assert!(
            report["providers"]
                .as_array()
                .unwrap()
                .iter()
                .all(|provider| provider["enabled"] == true)
        );
    }

    #[test]
    fn rollback_restores_upstream_resolution_offline() {
        let fixture = Fixture::new();
        fixture.activate();
        let installed = fs::read(fixture.layout.installed_updater()).unwrap();
        let upstream = fs::read(fixture.layout.upstream_root.join(UPDATER)).unwrap();
        rollback(&fixture.layout).unwrap();
        assert!(!fixture.layout.shim().exists());
        assert!(!fixture.layout.activation().exists());
        assert_eq!(
            fs::read(fixture.layout.installed_updater()).unwrap(),
            installed
        );
        assert_eq!(
            fs::read(fixture.layout.upstream_root.join(UPDATER)).unwrap(),
            upstream
        );
        assert_eq!(
            resolve_command(&fixture.path, UPDATER),
            Some(fixture.layout.upstream_root.join(UPDATER))
        );
    }
}
