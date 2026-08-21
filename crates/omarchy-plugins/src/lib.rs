use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

const SCHEMA_VERSION: u32 = 1;
const RUST_BADGE: &str = include_str!("../../../plugins/common/RustBadge.qml");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Component {
    Cleaner,
    Skills,
    NetworkInspector,
}

impl Component {
    const ALL: [Self; 3] = [Self::Cleaner, Self::Skills, Self::NetworkInspector];

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "cleaner" => Ok(Self::Cleaner),
            "skills" => Ok(Self::Skills),
            "network-inspector" | "network" => Ok(Self::NetworkInspector),
            _ => Err(format!("unknown-plugin: {value}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Cleaner => "cleaner",
            Self::Skills => "skills",
            Self::NetworkInspector => "network-inspector",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Cleaner => "omarchy-rs.cleaner",
            Self::Skills => "omarchy-rs.skills",
            Self::NetworkInspector => "omarchy-rs.network-inspector",
        }
    }

    fn files(self) -> [(&'static str, &'static str); 3] {
        match self {
            Self::Cleaner => [
                (
                    "manifest.json",
                    include_str!("../../../plugins/omarchy-rs.cleaner/manifest.json"),
                ),
                (
                    "Panel.qml",
                    include_str!("../../../plugins/omarchy-rs.cleaner/Panel.qml"),
                ),
                ("RustBadge.qml", RUST_BADGE),
            ],
            Self::Skills => [
                (
                    "manifest.json",
                    include_str!("../../../plugins/omarchy-rs.skills/manifest.json"),
                ),
                (
                    "Panel.qml",
                    include_str!("../../../plugins/omarchy-rs.skills/Panel.qml"),
                ),
                ("RustBadge.qml", RUST_BADGE),
            ],
            Self::NetworkInspector => [
                (
                    "manifest.json",
                    include_str!("../../../plugins/omarchy-rs.network-inspector/manifest.json"),
                ),
                (
                    "Panel.qml",
                    include_str!("../../../plugins/omarchy-rs.network-inspector/Panel.qml"),
                ),
                ("RustBadge.qml", RUST_BADGE),
            ],
        }
    }
}

#[derive(Clone, Debug)]
pub struct PluginLayout {
    pub config_root: PathBuf,
    pub path: Vec<PathBuf>,
    pub omarchy: PathBuf,
    pub restart_shell: PathBuf,
}

impl PluginLayout {
    pub fn from_environment() -> Result<Self, String> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is unset")?;
        Ok(Self {
            config_root: env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config")),
            path: env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect(),
            omarchy: env::var_os("OMARCHY_RS_OMARCHY")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("omarchy")),
            restart_shell: env::var_os("OMARCHY_RS_RESTART_SHELL")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("omarchy-restart-shell")),
        })
    }

    fn plugins_root(&self) -> PathBuf {
        self.config_root.join("omarchy/plugins")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Owner {
    schema_version: u32,
    plugin_id: String,
    files: Vec<OwnedFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct OwnedFile {
    name: String,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRecord {
    component: String,
    plugin_id: String,
    version: String,
    installed: bool,
    owned: bool,
    current: bool,
    enabled: Option<bool>,
    dependency_ready: bool,
    problems: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginReport {
    schema_version: u32,
    version: String,
    omarchy_available: bool,
    plugins: Vec<PluginRecord>,
    problems: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationReport {
    action: String,
    components: Vec<String>,
    restart_recommended: bool,
    restarted: bool,
}

fn executable(layout: &PluginLayout, value: &Path) -> Option<PathBuf> {
    if value.is_absolute() || value.components().count() > 1 {
        return is_executable(value).then(|| value.to_path_buf());
    }
    layout
        .path
        .iter()
        .map(|root| root.join(value))
        .find(|path| is_executable(path))
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .ok()
        .is_some_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn enabled_plugins(layout: &PluginLayout) -> Result<BTreeMap<String, bool>, String> {
    let omarchy = executable(layout, &layout.omarchy).ok_or("omarchy-unavailable")?;
    let output = Command::new(omarchy)
        .args(["plugin", "list", "--json"])
        .output()
        .map_err(|error| format!("omarchy-list-failed: {error}"))?;
    if !output.status.success() {
        return Err("omarchy-list-failed".into());
    }
    let values: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).map_err(|_| "omarchy-list-invalid-json")?;
    Ok(values
        .into_iter()
        .filter_map(|value| {
            Some((
                value.get("id")?.as_str()?.to_owned(),
                value.get("enabled")?.as_bool()?,
            ))
        })
        .collect())
}

fn dependency_ready(layout: &PluginLayout, component: Component) -> bool {
    component != Component::NetworkInspector || executable(layout, Path::new("sniffnet")).is_some()
}

pub fn inventory(layout: &PluginLayout) -> PluginReport {
    let enabled = enabled_plugins(layout);
    let mut global_problems = Vec::new();
    if let Err(error) = &enabled {
        global_problems.push(error.clone());
    }
    let plugins = Component::ALL
        .into_iter()
        .map(|component| {
            let destination = layout.plugins_root().join(component.id());
            let installed = destination.exists();
            let verification = installed.then(|| verify(&destination, component));
            let owned = verification.as_ref().is_some_and(|result| result.is_ok());
            let mut problems = Vec::new();
            let current = if let Some(Err(error)) = &verification {
                problems.push(format!("ownership-error:{error}"));
                false
            } else if let Some(Ok(owner)) = &verification {
                let value = owner_is_current(owner, component);
                if !value {
                    problems.push("stale-plugin".into());
                }
                value
            } else {
                false
            };
            let dependency_ready = dependency_ready(layout, component);
            if !dependency_ready {
                problems.push("missing-dependency:sniffnet".into());
            }
            PluginRecord {
                component: component.name().into(),
                plugin_id: component.id().into(),
                version: env!("CARGO_PKG_VERSION").into(),
                installed,
                owned,
                current,
                enabled: enabled
                    .as_ref()
                    .ok()
                    .and_then(|map| map.get(component.id()).copied()),
                dependency_ready,
                problems,
            }
        })
        .collect();
    PluginReport {
        schema_version: SCHEMA_VERSION,
        version: env!("CARGO_PKG_VERSION").into(),
        omarchy_available: enabled.is_ok(),
        plugins,
        problems: global_problems,
    }
}

fn expected_owner(component: Component) -> Owner {
    Owner {
        schema_version: SCHEMA_VERSION,
        plugin_id: component.id().into(),
        files: component
            .files()
            .iter()
            .map(|(name, content)| OwnedFile {
                name: (*name).into(),
                sha256: sha256(content.as_bytes()),
            })
            .collect(),
    }
}

fn owner_is_current(owner: &Owner, component: Component) -> bool {
    let expected = expected_owner(component);
    owner.files.len() == expected.files.len()
        && owner
            .files
            .iter()
            .zip(expected.files)
            .all(|(left, right)| left.name == right.name && left.sha256 == right.sha256)
}

fn verify(destination: &Path, component: Component) -> Result<Owner, String> {
    let owner: Owner = serde_json::from_slice(
        &fs::read(destination.join(".omarchy-rs-owner.json")).map_err(|_| "foreign-plugin")?,
    )
    .map_err(|_| "invalid-owner")?;
    if owner.schema_version != SCHEMA_VERSION || owner.plugin_id != component.id() {
        return Err("mismatched-owner".into());
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
            return Err("foreign-file".into());
        }
    }
    for file in &owner.files {
        let bytes = fs::read(destination.join(&file.name)).map_err(|_| "owned-file-missing")?;
        if sha256(&bytes) != file.sha256 {
            return Err("owned-file-modified".into());
        }
    }
    Ok(owner)
}

fn write_component(
    layout: &PluginLayout,
    component: Component,
    require_dependency: bool,
) -> Result<(), String> {
    if require_dependency
        && component == Component::NetworkInspector
        && !dependency_ready(layout, component)
    {
        return Err("missing-dependency:sniffnet".into());
    }
    let destination = layout.plugins_root().join(component.id());
    if destination.exists() {
        verify(&destination, component)?;
    } else {
        fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    }
    for (name, content) in component.files() {
        atomic_bytes(&destination.join(name), content.as_bytes())?;
    }
    atomic_bytes(
        &destination.join(".omarchy-rs-owner.json"),
        &serde_json::to_vec_pretty(&expected_owner(component)).map_err(|e| e.to_string())?,
    )
}

fn remove_component(layout: &PluginLayout, component: Component) -> Result<(), String> {
    let destination = layout.plugins_root().join(component.id());
    let owner = verify(&destination, component)?;
    run_omarchy(layout, "disable", component.id())?;
    for file in owner.files {
        fs::remove_file(destination.join(file.name)).map_err(|error| error.to_string())?;
    }
    fs::remove_file(destination.join(".omarchy-rs-owner.json")).map_err(|e| e.to_string())?;
    fs::remove_dir(destination).map_err(|_| "foreign-file".to_owned())
}

fn run_omarchy(layout: &PluginLayout, action: &str, id: &str) -> Result<(), String> {
    let omarchy = executable(layout, &layout.omarchy).ok_or("omarchy-unavailable")?;
    let status = Command::new(omarchy)
        .args(["plugin", action, id])
        .status()
        .map_err(|error| format!("omarchy-{action}-failed:{error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("omarchy-{action}-failed"))
}

fn restart_shell(layout: &PluginLayout) -> Result<(), String> {
    let executable =
        executable(layout, &layout.restart_shell).ok_or("restart-shell-unavailable")?;
    let status = Command::new(executable)
        .status()
        .map_err(|error| format!("restart-shell-failed:{error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "restart-shell-failed".into())
}

fn mutate(action: &str, components: Vec<Component>, restarted: bool) -> MutationReport {
    MutationReport {
        action: action.into(),
        components: components
            .into_iter()
            .map(|value| value.name().into())
            .collect(),
        restart_recommended: !restarted,
        restarted,
    }
}

fn atomic_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("invalid-plugin-path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = parent.join(format!(".plugin-tmp-{}", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map_err(|error| error.to_string())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|error| error.to_string())
}

pub fn execute_cli(args: &[String]) -> Result<String, String> {
    let layout = PluginLayout::from_environment()?;
    execute(&layout, args)
}

fn execute(layout: &PluginLayout, args: &[String]) -> Result<String, String> {
    match args {
        [command] if command == "list" || command == "doctor" => json(&inventory(layout)),
        [command, flag] if (command == "list" || command == "doctor") && flag == "--json" => {
            json(&inventory(layout))
        }
        [command, component] if command == "install" => {
            let component = Component::parse(component)?;
            write_component(layout, component, true)?;
            json(&mutate("installed", vec![component], false))
        }
        [command, component] if command == "enable" => {
            let component = Component::parse(component)?;
            verify(&layout.plugins_root().join(component.id()), component)?;
            run_omarchy(layout, "enable", component.id())?;
            json(&mutate("enabled", vec![component], false))
        }
        [command] | [command, _] | [command, _, _] if command == "update" => {
            let (selected, restart) = match args {
                [_] => (None, false),
                [_, flag] if flag == "--restart" => (None, true),
                [_, component] => (Some(Component::parse(component)?), false),
                [_, component, flag] if flag == "--restart" => {
                    (Some(Component::parse(component)?), true)
                }
                _ => return Err(plugin_usage()),
            };
            let installed = Component::ALL
                .into_iter()
                .filter(|component| selected.is_none_or(|selected| selected == *component))
                .filter(|component| layout.plugins_root().join(component.id()).exists())
                .collect::<Vec<_>>();
            if selected.is_some() && installed.is_empty() {
                return Err("plugin-not-installed".into());
            }
            for component in &installed {
                verify(&layout.plugins_root().join(component.id()), *component)?;
            }
            for component in &installed {
                write_component(layout, *component, false)?;
            }
            if restart {
                restart_shell(layout)?;
            }
            json(&mutate("updated", installed, restart))
        }
        [command, component] if command == "uninstall" => {
            let component = Component::parse(component)?;
            remove_component(layout, component)?;
            json(&mutate("uninstalled", vec![component], false))
        }
        _ => Err(plugin_usage()),
    }
}

fn plugin_usage() -> String {
    "usage: omarchy-rs plugin <list|doctor|install COMPONENT|enable COMPONENT|update [COMPONENT] [--restart]|uninstall COMPONENT> [--json]".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct Fixture {
        _temp: TempDir,
        layout: PluginLayout,
        receipt: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();
            let bin = temp.path().join("bin");
            fs::create_dir_all(&bin).unwrap();
            let receipt = temp.path().join("omarchy.argv");
            executable_file(
                &bin.join("omarchy"),
                &format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nif [ \"$1 $2\" = 'plugin list' ]; then printf '[{{\"id\":\"omarchy-rs.cleaner\",\"enabled\":true}}]'; fi\n",
                    receipt.display()
                ),
            );
            executable_file(&bin.join("sniffnet"), "#!/bin/sh\nexit 0\n");
            Self {
                layout: PluginLayout {
                    config_root: temp.path().join("config"),
                    path: vec![bin],
                    omarchy: PathBuf::from("omarchy"),
                    restart_shell: PathBuf::from("restart-shell"),
                },
                receipt,
                _temp: temp,
            }
        }
    }

    fn executable_file(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn plugins_inventory_normalizes_owned_components() {
        let fixture = Fixture::new();
        write_component(&fixture.layout, Component::Cleaner, true).unwrap();
        let report = inventory(&fixture.layout);
        assert_eq!(report.plugins.len(), 3);
        let cleaner = &report.plugins[0];
        assert!(cleaner.installed && cleaner.owned && cleaner.current);
        assert_eq!(cleaner.enabled, Some(true));
        assert_eq!(cleaner.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn plugins_doctor_reports_actionable_drift() {
        let fixture = Fixture::new();
        write_component(&fixture.layout, Component::Cleaner, true).unwrap();
        let destination = fixture.layout.plugins_root().join(Component::Cleaner.id());
        let mut owner: Owner =
            serde_json::from_slice(&fs::read(destination.join(".omarchy-rs-owner.json")).unwrap())
                .unwrap();
        fs::write(destination.join("manifest.json"), "{\"old\":true}").unwrap();
        owner.files[0].sha256 = sha256(b"{\"old\":true}");
        atomic_bytes(
            &destination.join(".omarchy-rs-owner.json"),
            &serde_json::to_vec_pretty(&owner).unwrap(),
        )
        .unwrap();
        fs::remove_file(fixture.layout.path[0].join("sniffnet")).unwrap();
        let report = inventory(&fixture.layout);
        assert!(report.plugins[0].problems.contains(&"stale-plugin".into()));
        assert!(
            report.plugins[2]
                .problems
                .contains(&"missing-dependency:sniffnet".into())
        );
    }

    #[test]
    fn plugins_install_is_owned_and_requests_restart() {
        let fixture = Fixture::new();
        let output = execute(&fixture.layout, &["install".into(), "skills".into()]).unwrap();
        assert!(output.contains("\"restartRecommended\": true"));
        assert!(
            verify(
                &fixture.layout.plugins_root().join(Component::Skills.id()),
                Component::Skills
            )
            .is_ok()
        );
    }

    #[test]
    fn plugins_mutations_refuse_foreign_or_modified_files() {
        let fixture = Fixture::new();
        let destination = fixture.layout.plugins_root().join(Component::Skills.id());
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("mine"), "keep").unwrap();
        assert!(write_component(&fixture.layout, Component::Skills, true).is_err());
        assert_eq!(
            fs::read_to_string(destination.join("mine")).unwrap(),
            "keep"
        );
        fs::remove_dir_all(&destination).unwrap();
        write_component(&fixture.layout, Component::Skills, true).unwrap();
        fs::write(destination.join("Panel.qml"), "changed").unwrap();
        assert!(write_component(&fixture.layout, Component::Skills, true).is_err());
        assert!(remove_component(&fixture.layout, Component::Skills).is_err());
    }

    #[test]
    fn plugins_bulk_update_refreshes_installed_only() {
        let fixture = Fixture::new();
        write_component(&fixture.layout, Component::Cleaner, true).unwrap();
        let output = execute(&fixture.layout, &["update".into()]).unwrap();
        assert!(output.contains("cleaner"));
        assert!(
            !fixture
                .layout
                .plugins_root()
                .join(Component::Skills.id())
                .exists()
        );
        assert!(
            !fixture
                .layout
                .plugins_root()
                .join(Component::NetworkInspector.id())
                .exists()
        );
    }

    #[test]
    fn plugins_update_restart_is_explicit_and_ordered() {
        let fixture = Fixture::new();
        let restart = fixture.layout.path[0].join("restart-shell");
        let marker = fixture.receipt.with_extension("restart");
        executable_file(
            &restart,
            &format!("#!/bin/sh\nprintf restarted > '{}'\n", marker.display()),
        );
        write_component(&fixture.layout, Component::Cleaner, true).unwrap();
        let normal = execute(&fixture.layout, &["update".into()]).unwrap();
        assert!(normal.contains("\"restartRecommended\": true"));
        assert!(normal.contains("\"restarted\": false"));
        assert!(!marker.exists());
        let restarted = execute(
            &fixture.layout,
            &["update".into(), "cleaner".into(), "--restart".into()],
        )
        .unwrap();
        assert!(restarted.contains("\"restartRecommended\": false"));
        assert!(restarted.contains("\"restarted\": true"));
        assert_eq!(fs::read_to_string(marker).unwrap(), "restarted");
        assert!(
            inventory(&fixture.layout)
                .plugins
                .first()
                .is_some_and(|plugin| plugin.current)
        );
    }

    #[test]
    fn plugins_enable_and_uninstall_use_direct_omarchy_argv() {
        let fixture = Fixture::new();
        write_component(&fixture.layout, Component::Cleaner, true).unwrap();
        execute(&fixture.layout, &["enable".into(), "cleaner".into()]).unwrap();
        execute(&fixture.layout, &["uninstall".into(), "cleaner".into()]).unwrap();
        let receipt = fs::read_to_string(fixture.receipt).unwrap();
        assert!(receipt.contains("plugin enable omarchy-rs.cleaner"));
        assert!(receipt.contains("plugin disable omarchy-rs.cleaner"));
        assert!(!receipt.contains("sh -c"));
    }

    #[test]
    fn plugins_reject_unknown_component_or_missing_omarchy() {
        let mut fixture = Fixture::new();
        assert!(execute(&fixture.layout, &["install".into(), "unknown".into()]).is_err());
        write_component(&fixture.layout, Component::Cleaner, true).unwrap();
        fixture.layout.omarchy = PathBuf::from("missing-omarchy");
        assert!(execute(&fixture.layout, &["enable".into(), "cleaner".into()]).is_err());
        assert!(
            fixture
                .layout
                .plugins_root()
                .join(Component::Cleaner.id())
                .exists()
        );
    }
}
