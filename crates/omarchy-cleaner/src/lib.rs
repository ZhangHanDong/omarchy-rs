use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env, fs,
    io::Write,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const SCHEMA_VERSION: u32 = 1;
pub const RECENT_WRITE_GUARD: Duration = Duration::from_secs(300);
const PLUGIN_ID: &str = "omarchy-rs.cleaner";
const PLUGIN_MANIFEST: &str = include_str!("../../../plugins/omarchy-rs.cleaner/manifest.json");
const PLUGIN_PANEL: &str = include_str!("../../../plugins/omarchy-rs.cleaner/Panel.qml");
const RUST_BADGE: &str = include_str!("../../../plugins/common/RustBadge.qml");

#[derive(Clone, Debug)]
pub struct CleanerLayout {
    pub home: PathBuf,
    pub state_root: PathBuf,
    pub config_root: PathBuf,
}

impl CleanerLayout {
    pub fn from_environment() -> Result<Self, String> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is unset")?;
        let state_root = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let config_root = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        Ok(Self {
            home,
            state_root,
            config_root,
        })
    }

    pub fn default_root(&self) -> PathBuf {
        self.home.join("Work")
    }

    fn plans_dir(&self) -> PathBuf {
        self.state_root.join("omarchy-rs/cleaner/plans")
    }

    fn plan_path(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("invalid cleanup plan id".into());
        }
        Ok(self.plans_dir().join(format!("{id}.json")))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    RustTarget,
    NodeModules,
    NextCache,
    TurboCache,
    ViteCache,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub path: PathBuf,
    pub project_root: PathBuf,
    pub kind: ArtifactKind,
    pub evidence: Vec<String>,
    pub bytes: u64,
    pub files: u64,
    pub device: u64,
    pub inode: u64,
    pub owner_uid: u32,
    pub latest_write_unix_ms: u64,
    pub eligible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub schema_version: u32,
    pub root: PathBuf,
    pub candidates: Vec<Candidate>,
    pub total_bytes: u64,
    pub total_files: u64,
    pub skipped_boundaries: u64,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlan {
    pub schema_version: u32,
    pub id: String,
    pub confirmation_token: String,
    pub root: PathBuf,
    pub created_unix_ms: u64,
    pub candidates: Vec<Candidate>,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyItem {
    pub id: String,
    pub path: PathBuf,
    pub status: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub schema_version: u32,
    pub plan_id: String,
    pub reclaimed_bytes: u64,
    pub removed: u64,
    pub skipped: u64,
    pub failed: u64,
    pub items: Vec<ApplyItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerBenchmarkConfig {
    pub fixture_version: String,
    pub projects: u32,
    pub files_per_artifact: u32,
    pub bytes_per_file: u32,
    pub warmups: u32,
    pub samples: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerBenchmarkMetrics {
    pub median_wall_ms: f64,
    pub median_cpu_ms: f64,
    pub max_rss_kib: u64,
    pub median_read_bytes: Option<u64>,
    pub median_written_bytes: Option<u64>,
    pub child_process_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerBenchmarkReport {
    pub schema_version: u32,
    pub config: CleanerBenchmarkConfig,
    pub python: CleanerBenchmarkMetrics,
    pub rust: CleanerBenchmarkMetrics,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkEligibility {
    pub default_enabled: bool,
    pub reasons: Vec<String>,
}

pub fn validate_benchmark_report(report: &CleanerBenchmarkReport) -> Result<(), String> {
    if report.schema_version != SCHEMA_VERSION {
        return Err("unsupported cleaner benchmark schema".into());
    }
    if report.config.fixture_version.is_empty()
        || report.config.projects == 0
        || report.config.files_per_artifact == 0
        || report.config.bytes_per_file == 0
        || report.config.warmups != 3
        || report.config.samples != 30
    {
        return Err("cleaner benchmark configuration is incomplete".into());
    }
    for (name, metrics) in [("python", &report.python), ("rust", &report.rust)] {
        if !metrics.median_wall_ms.is_finite()
            || metrics.median_wall_ms <= 0.0
            || !metrics.median_cpu_ms.is_finite()
            || metrics.median_cpu_ms < 0.0
            || metrics.max_rss_kib == 0
        {
            return Err(format!("{name} benchmark metrics are incomplete"));
        }
    }
    Ok(())
}

pub fn benchmark_eligibility(report: &CleanerBenchmarkReport) -> BenchmarkEligibility {
    let mut reasons = Vec::new();
    if let Err(error) = validate_benchmark_report(report) {
        reasons.push(error);
        return BenchmarkEligibility {
            default_enabled: false,
            reasons,
        };
    }
    let wall_ratio = report.rust.median_wall_ms / report.python.median_wall_ms;
    let rss_ratio = report.rust.max_rss_kib as f64 / report.python.max_rss_kib as f64;
    if wall_ratio > 0.6 && rss_ratio > 0.6 {
        reasons.push("neither wall time nor peak RSS improves by 40%".into());
    }
    if wall_ratio > 1.1 {
        reasons.push("wall-time regression exceeds 10%".into());
    }
    if rss_ratio > 1.1 {
        reasons.push("peak-RSS regression exceeds 10%".into());
    }
    BenchmarkEligibility {
        default_enabled: reasons.is_empty(),
        reasons,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginOwner {
    schema_version: u32,
    plugin_id: String,
    files: Vec<PluginFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PluginFile {
    name: String,
    sha256: String,
}

#[derive(Default)]
struct Measure {
    bytes: u64,
    files: u64,
    latest_write_ms: u64,
    skipped_boundaries: u64,
}

pub fn scan(layout: &CleanerLayout, root: &Path) -> Result<ScanReport, String> {
    scan_at(layout, root, SystemTime::now(), RECENT_WRITE_GUARD)
}

fn scan_at(
    layout: &CleanerLayout,
    root: &Path,
    now: SystemTime,
    recent_guard: Duration,
) -> Result<ScanReport, String> {
    let (home, root, owner_uid, root_device) = validate_root(layout, root)?;
    let mut candidates = Vec::new();
    let mut warnings = Vec::new();
    let mut skipped_boundaries = 0;
    let mut stack = vec![root.clone()];

    while let Some(directory) = stack.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                warnings.push(format!("{}: {error}", directory.display()));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    warnings.push(error.to_string());
                    continue;
                }
            };
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    warnings.push(format!("{}: {error}", path.display()));
                    continue;
                }
            };
            if metadata.file_type().is_symlink() {
                skipped_boundaries += 1;
                continue;
            }
            if !metadata.is_dir() {
                continue;
            }
            if metadata.dev() != root_device {
                skipped_boundaries += 1;
                continue;
            }
            if metadata.uid() != owner_uid {
                skipped_boundaries += 1;
                continue;
            }
            if path.file_name().and_then(|name| name.to_str()) == Some(".git") {
                skipped_boundaries += 1;
                continue;
            }

            if let Some((kind, project_root, evidence)) = classify(&path) {
                let measure = measure_dir(&path, root_device, owner_uid, &mut warnings)?;
                skipped_boundaries += measure.skipped_boundaries;
                let age = now
                    .duration_since(UNIX_EPOCH + Duration::from_millis(measure.latest_write_ms))
                    .unwrap_or_default();
                let eligible = age >= recent_guard;
                let canonical_path = path.canonicalize().map_err(|error| error.to_string())?;
                if !canonical_path.starts_with(&root) || !canonical_path.starts_with(&home) {
                    skipped_boundaries += 1;
                    continue;
                }
                candidates.push(Candidate {
                    id: candidate_id(&canonical_path, &kind, metadata.dev(), metadata.ino()),
                    path: canonical_path,
                    project_root: project_root
                        .canonicalize()
                        .map_err(|error| error.to_string())?,
                    kind,
                    evidence,
                    bytes: measure.bytes,
                    files: measure.files,
                    device: metadata.dev(),
                    inode: metadata.ino(),
                    owner_uid: metadata.uid(),
                    latest_write_unix_ms: measure.latest_write_ms,
                    eligible,
                    blocked_reason: (!eligible).then(|| "recent-write".into()),
                });
                continue;
            }
            stack.push(path);
        }
    }

    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    let total_bytes = candidates.iter().map(|candidate| candidate.bytes).sum();
    let total_files = candidates.iter().map(|candidate| candidate.files).sum();
    Ok(ScanReport {
        schema_version: SCHEMA_VERSION,
        root,
        candidates,
        total_bytes,
        total_files,
        skipped_boundaries,
        warnings,
    })
}

fn validate_root(
    layout: &CleanerLayout,
    requested: &Path,
) -> Result<(PathBuf, PathBuf, u32, u64), String> {
    if !requested.is_absolute() {
        return Err("cleanup root must be absolute".into());
    }
    let home = layout
        .home
        .canonicalize()
        .map_err(|error| format!("cannot resolve HOME: {error}"))?;
    let root = requested
        .canonicalize()
        .map_err(|error| format!("cannot resolve cleanup root: {error}"))?;
    if root == Path::new("/") || root == home || !root.starts_with(&home) {
        return Err("cleanup root must be a directory below HOME".into());
    }
    let home_metadata = fs::metadata(&home).map_err(|error| error.to_string())?;
    let root_metadata = fs::metadata(&root).map_err(|error| error.to_string())?;
    if !root_metadata.is_dir() {
        return Err("cleanup root is not a directory".into());
    }
    if root_metadata.uid() != home_metadata.uid() {
        return Err("cleanup root is not owned by the current home owner".into());
    }
    Ok((home, root, home_metadata.uid(), root_metadata.dev()))
}

fn classify(path: &Path) -> Option<(ArtifactKind, PathBuf, Vec<String>)> {
    let name = path.file_name()?.to_str()?;
    let parent = path.parent()?;
    match name {
        "target"
            if parent.join("Cargo.toml").is_file()
                && [".rustc_info.json", "CACHEDIR.TAG", "debug", "release"]
                    .iter()
                    .any(|marker| path.join(marker).exists()) =>
        {
            Some((
                ArtifactKind::RustTarget,
                parent.into(),
                vec!["Cargo.toml".into(), "cargo-target-marker".into()],
            ))
        }
        "node_modules" if parent.join("package.json").is_file() => Some((
            ArtifactKind::NodeModules,
            parent.into(),
            vec!["package.json".into(), "node_modules".into()],
        )),
        "cache"
            if parent.file_name().and_then(|value| value.to_str()) == Some(".next")
                && parent.parent()?.join("package.json").is_file() =>
        {
            Some((
                ArtifactKind::NextCache,
                parent.parent()?.into(),
                vec!["package.json".into(), ".next/cache".into()],
            ))
        }
        ".turbo" if parent.join("package.json").is_file() => Some((
            ArtifactKind::TurboCache,
            parent.into(),
            vec!["package.json".into(), ".turbo".into()],
        )),
        ".vite"
            if parent.file_name().and_then(|value| value.to_str()) == Some("node_modules")
                && parent.parent()?.join("package.json").is_file() =>
        {
            Some((
                ArtifactKind::ViteCache,
                parent.parent()?.into(),
                vec!["package.json".into(), "node_modules/.vite".into()],
            ))
        }
        _ => None,
    }
}

fn measure_dir(
    root: &Path,
    device: u64,
    owner_uid: u32,
    warnings: &mut Vec<String>,
) -> Result<Measure, String> {
    let mut measure = Measure::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                warnings.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if metadata.file_type().is_symlink()
            || metadata.dev() != device
            || metadata.uid() != owner_uid
        {
            measure.skipped_boundaries += 1;
            continue;
        }
        measure.latest_write_ms = measure.latest_write_ms.max(modified_ms(&metadata));
        if metadata.is_file() {
            measure.bytes = measure.bytes.saturating_add(metadata.len());
            measure.files += 1;
            continue;
        }
        if metadata.is_dir() {
            let entries = match fs::read_dir(&path) {
                Ok(entries) => entries,
                Err(error) => {
                    warnings.push(format!("{}: {error}", path.display()));
                    continue;
                }
            };
            for entry in entries.flatten() {
                stack.push(entry.path());
            }
        }
    }
    Ok(measure)
}

fn modified_ms(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

fn candidate_id(path: &Path, kind: &ArtifactKind, device: u64, inode: u64) -> String {
    let material = format!("{}|{kind:?}|{device}|{inode}", path.display());
    format!("{:x}", Sha256::digest(material.as_bytes()))[..16].into()
}

pub fn create_plan(
    layout: &CleanerLayout,
    root: &Path,
    selected_ids: &[String],
) -> Result<CleanupPlan, String> {
    create_plan_at(
        layout,
        root,
        selected_ids,
        SystemTime::now(),
        RECENT_WRITE_GUARD,
    )
}

fn create_plan_at(
    layout: &CleanerLayout,
    root: &Path,
    selected_ids: &[String],
    now: SystemTime,
    recent_guard: Duration,
) -> Result<CleanupPlan, String> {
    if selected_ids.is_empty() {
        return Err("cleanup plan requires at least one candidate".into());
    }
    let selected = selected_ids.iter().collect::<BTreeSet<_>>();
    if selected.len() != selected_ids.len() {
        return Err("cleanup plan contains duplicate candidates".into());
    }
    let report = scan_at(layout, root, now, recent_guard)?;
    let candidates = report
        .candidates
        .into_iter()
        .filter(|candidate| selected.contains(&candidate.id) && candidate.eligible)
        .collect::<Vec<_>>();
    if candidates.len() != selected.len() {
        return Err("one or more selected candidates are missing or ineligible".into());
    }
    let created_unix_ms = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64;
    let seed = serde_json::to_vec(&(SCHEMA_VERSION, &report.root, created_unix_ms, &candidates))
        .map_err(|error| error.to_string())?;
    let digest = format!("{:x}", Sha256::digest(&seed));
    let id = digest[..16].to_string();
    let confirmation_token = digest[16..48].to_string();
    let plan = CleanupPlan {
        schema_version: SCHEMA_VERSION,
        id: id.clone(),
        confirmation_token,
        root: report.root,
        total_bytes: candidates.iter().map(|candidate| candidate.bytes).sum(),
        created_unix_ms,
        candidates,
    };
    atomic_json(&layout.plan_path(&id)?, &plan)?;
    Ok(plan)
}

pub fn apply_plan(
    layout: &CleanerLayout,
    plan_id: &str,
    confirmation_token: &str,
) -> Result<ApplyReport, String> {
    apply_plan_at(layout, plan_id, confirmation_token, SystemTime::now())
}

fn apply_plan_at(
    layout: &CleanerLayout,
    plan_id: &str,
    confirmation_token: &str,
    now: SystemTime,
) -> Result<ApplyReport, String> {
    let plan_path = layout.plan_path(plan_id)?;
    let plan: CleanupPlan = serde_json::from_slice(
        &fs::read(&plan_path).map_err(|error| format!("cannot read cleanup plan: {error}"))?,
    )
    .map_err(|error| format!("invalid cleanup plan: {error}"))?;
    if plan.schema_version != SCHEMA_VERSION || plan.id != plan_id {
        return Err("cleanup plan identity is invalid".into());
    }
    if confirmation_token.is_empty() || confirmation_token != plan.confirmation_token {
        return Err("cleanup confirmation token does not match".into());
    }
    let (_, root, owner_uid, root_device) = validate_root(layout, &plan.root)?;
    let mut report = ApplyReport {
        schema_version: SCHEMA_VERSION,
        plan_id: plan.id.clone(),
        reclaimed_bytes: 0,
        removed: 0,
        skipped: 0,
        failed: 0,
        items: Vec::new(),
    };

    for candidate in &plan.candidates {
        let result = revalidate(candidate, &root, root_device, owner_uid, now);
        if let Err(reason) = result {
            report.skipped += 1;
            report.items.push(ApplyItem {
                id: candidate.id.clone(),
                path: candidate.path.clone(),
                status: "skipped".into(),
                bytes: 0,
                reason: Some(reason),
            });
            continue;
        }
        match fs::remove_dir_all(&candidate.path) {
            Ok(()) => {
                report.removed += 1;
                report.reclaimed_bytes = report.reclaimed_bytes.saturating_add(candidate.bytes);
                report.items.push(ApplyItem {
                    id: candidate.id.clone(),
                    path: candidate.path.clone(),
                    status: "removed".into(),
                    bytes: candidate.bytes,
                    reason: None,
                });
            }
            Err(error) => {
                report.failed += 1;
                report.items.push(ApplyItem {
                    id: candidate.id.clone(),
                    path: candidate.path.clone(),
                    status: "failed".into(),
                    bytes: 0,
                    reason: Some(error.to_string()),
                });
            }
        }
    }
    atomic_json(&plan_path, &plan)?;
    Ok(report)
}

fn revalidate(
    candidate: &Candidate,
    root: &Path,
    root_device: u64,
    owner_uid: u32,
    now: SystemTime,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(&candidate.path)
        .map_err(|_| "candidate-missing-or-unreadable".to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("candidate-type-changed".into());
    }
    if metadata.dev() != root_device
        || metadata.dev() != candidate.device
        || metadata.ino() != candidate.inode
        || metadata.uid() != owner_uid
        || metadata.uid() != candidate.owner_uid
    {
        return Err("identity-mismatch".into());
    }
    let canonical = candidate
        .path
        .canonicalize()
        .map_err(|_| "candidate-canonicalization-failed".to_string())?;
    if canonical != candidate.path || !canonical.starts_with(root) || canonical == root {
        return Err("outside-plan-boundary".into());
    }
    let Some((kind, project_root, _)) = classify(&canonical) else {
        return Err("project-evidence-missing".into());
    };
    if kind != candidate.kind
        || project_root.canonicalize().ok().as_deref() != Some(candidate.project_root.as_path())
    {
        return Err("project-evidence-changed".into());
    }
    let mut warnings = Vec::new();
    let measured = measure_dir(&canonical, root_device, owner_uid, &mut warnings)?;
    if measured.skipped_boundaries > 0 || !warnings.is_empty() {
        return Err("candidate-boundary-unverifiable".into());
    }
    if measured.latest_write_ms > candidate.latest_write_unix_ms
        || measured.bytes != candidate.bytes
        || measured.files != candidate.files
    {
        return Err("recent-write".into());
    }
    let latest = UNIX_EPOCH + Duration::from_millis(measured.latest_write_ms);
    if now.duration_since(latest).unwrap_or_default() < RECENT_WRITE_GUARD {
        return Err("recent-write".into());
    }
    Ok(())
}

fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path.parent().ok_or("state path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|error| error.to_string())
}

pub fn install_plugin(layout: &CleanerLayout) -> Result<PathBuf, String> {
    let destination = layout.config_root.join("omarchy/plugins").join(PLUGIN_ID);
    if destination.exists() {
        verify_owned_plugin(&destination)?;
    } else {
        fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    }
    let sources = [
        ("manifest.json", PLUGIN_MANIFEST),
        ("Panel.qml", PLUGIN_PANEL),
        ("RustBadge.qml", RUST_BADGE),
    ];
    for (name, contents) in sources {
        atomic_bytes(&destination.join(name), contents.as_bytes())?;
    }
    let owner = PluginOwner {
        schema_version: SCHEMA_VERSION,
        plugin_id: PLUGIN_ID.into(),
        files: sources
            .iter()
            .map(|(name, contents)| PluginFile {
                name: (*name).into(),
                sha256: sha256(contents.as_bytes()),
            })
            .collect(),
    };
    atomic_json(&destination.join(".omarchy-rs-owner.json"), &owner)?;
    Ok(destination)
}

pub fn uninstall_plugin(layout: &CleanerLayout) -> Result<PathBuf, String> {
    let destination = layout.config_root.join("omarchy/plugins").join(PLUGIN_ID);
    let owner = verify_owned_plugin(&destination)?;
    for file in owner.files {
        fs::remove_file(destination.join(file.name)).map_err(|error| error.to_string())?;
    }
    fs::remove_file(destination.join(".omarchy-rs-owner.json"))
        .map_err(|error| error.to_string())?;
    fs::remove_dir(&destination).map_err(|error| {
        format!("plugin contains unowned files; refusing directory removal: {error}")
    })?;
    Ok(destination)
}

fn verify_owned_plugin(destination: &Path) -> Result<PluginOwner, String> {
    let marker = destination.join(".omarchy-rs-owner.json");
    let owner: PluginOwner = serde_json::from_slice(
        &fs::read(&marker).map_err(|_| "refusing foreign plugin without ownership marker")?,
    )
    .map_err(|_| "refusing plugin with invalid ownership marker")?;
    if owner.schema_version != SCHEMA_VERSION || owner.plugin_id != PLUGIN_ID {
        return Err("refusing plugin with mismatched ownership marker".into());
    }
    let owned_names = owner
        .files
        .iter()
        .map(|file| file.name.as_str())
        .chain(std::iter::once(".omarchy-rs-owner.json"))
        .collect::<BTreeSet<_>>();
    for entry in fs::read_dir(destination).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or("plugin contains a non-UTF-8 file name")?;
        if !owned_names.contains(name) {
            return Err(format!("refusing foreign plugin file: {name}"));
        }
    }
    for file in &owner.files {
        let path = destination.join(&file.name);
        let bytes =
            fs::read(&path).map_err(|_| format!("owned plugin file is missing: {}", file.name))?;
        if sha256(&bytes) != file.sha256 {
            return Err(format!("refusing modified plugin file: {}", file.name));
        }
    }
    Ok(owner)
}

fn atomic_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("output path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
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

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn execute_cli(args: &[String]) -> Result<String, String> {
    let layout = CleanerLayout::from_environment()?;
    let Some(command) = args.first().map(String::as_str) else {
        return Err(cleaner_usage());
    };
    match command {
        "scan" => {
            let (root, json) = parse_root_json(&layout, &args[1..])?;
            let report = scan(&layout, &root)?;
            if json {
                serde_json::to_string_pretty(&report).map_err(|error| error.to_string())
            } else {
                Ok(format!(
                    "{} candidates, {} bytes, {} files under {}",
                    report.candidates.len(),
                    report.total_bytes,
                    report.total_files,
                    report.root.display()
                ))
            }
        }
        "plan" => {
            let mut root = layout.default_root();
            let mut json = false;
            let mut candidates = Vec::new();
            let mut index = 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--root" if index + 1 < args.len() => {
                        root = expand_cli_root(&layout, &args[index + 1])?;
                        index += 2;
                    }
                    "--candidate" if index + 1 < args.len() => {
                        candidates.push(args[index + 1].clone());
                        index += 2;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    _ => return Err(cleaner_usage()),
                }
            }
            let plan = create_plan(&layout, &root, &candidates)?;
            if json {
                serde_json::to_string_pretty(&plan).map_err(|error| error.to_string())
            } else {
                Ok(format!(
                    "plan {}: {} candidates, {} bytes; confirm with {}",
                    plan.id,
                    plan.candidates.len(),
                    plan.total_bytes,
                    plan.confirmation_token
                ))
            }
        }
        "apply" => {
            let mut plan_id = None;
            let mut token = None;
            let mut json = false;
            let mut index = 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--plan" if index + 1 < args.len() => {
                        plan_id = Some(args[index + 1].clone());
                        index += 2;
                    }
                    "--confirm" if index + 1 < args.len() => {
                        token = Some(args[index + 1].clone());
                        index += 2;
                    }
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    _ => return Err(cleaner_usage()),
                }
            }
            let report = apply_plan(
                &layout,
                plan_id.as_deref().ok_or_else(cleaner_usage)?,
                token.as_deref().ok_or_else(cleaner_usage)?,
            )?;
            if json {
                serde_json::to_string_pretty(&report).map_err(|error| error.to_string())
            } else {
                Ok(format!(
                    "reclaimed {} bytes; {} removed, {} skipped, {} failed",
                    report.reclaimed_bytes, report.removed, report.skipped, report.failed
                ))
            }
        }
        "install-plugin" if args.len() == 1 => {
            Ok(format!("installed {}", install_plugin(&layout)?.display()))
        }
        "uninstall-plugin" if args.len() == 1 => Ok(format!(
            "uninstalled {}",
            uninstall_plugin(&layout)?.display()
        )),
        _ => Err(cleaner_usage()),
    }
}

fn parse_root_json(layout: &CleanerLayout, args: &[String]) -> Result<(PathBuf, bool), String> {
    let mut root = layout.default_root();
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" if index + 1 < args.len() => {
                root = expand_cli_root(layout, &args[index + 1])?;
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            _ => return Err(cleaner_usage()),
        }
    }
    Ok((root, json))
}

fn expand_cli_root(layout: &CleanerLayout, value: &str) -> Result<PathBuf, String> {
    if value == "~" {
        return Ok(layout.home.clone());
    }
    if let Some(relative) = value.strip_prefix("~/") {
        if relative.is_empty() {
            return Ok(layout.home.clone());
        }
        return Ok(layout.home.join(relative));
    }
    if value.starts_with('~') {
        return Err("cleanup root only supports ~ for the current user".into());
    }
    Ok(PathBuf::from(value))
}

fn cleaner_usage() -> String {
    "usage: omarchy-rs cleaner <scan [--root PATH] [--json]|plan [--root PATH] --candidate ID... [--json]|apply --plan ID --confirm TOKEN [--json]|install-plugin|uninstall-plugin>".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    struct Fixture {
        _temp: TempDir,
        layout: CleanerLayout,
        work: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let home = temp.path().join("home");
            let work = home.join("Work");
            fs::create_dir_all(&work).unwrap();
            Self {
                layout: CleanerLayout {
                    home,
                    state_root: temp.path().join("state"),
                    config_root: temp.path().join("config"),
                },
                work,
                _temp: temp,
            }
        }

        fn rust(&self, name: &str) -> PathBuf {
            let project = self.work.join(name);
            fs::create_dir_all(project.join("target/debug")).unwrap();
            fs::write(project.join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
            fs::write(project.join("target/.rustc_info.json"), "{}").unwrap();
            fs::write(project.join("target/debug/app"), b"rust-bytes").unwrap();
            project.join("target")
        }

        fn node(&self, name: &str) -> PathBuf {
            let project = self.work.join(name);
            fs::create_dir_all(project.join("node_modules/.vite")).unwrap();
            fs::write(project.join("package.json"), "{}").unwrap();
            fs::write(project.join("node_modules/pkg.js"), b"node-bytes").unwrap();
            fs::write(project.join("node_modules/.vite/cache"), b"vite").unwrap();
            project.join("node_modules")
        }

        fn old_now() -> SystemTime {
            SystemTime::now() + Duration::from_secs(600)
        }

        fn scan_old(&self) -> ScanReport {
            scan_at(
                &self.layout,
                &self.work,
                Self::old_now(),
                RECENT_WRITE_GUARD,
            )
            .unwrap()
        }
    }

    #[test]
    fn cleaner_scan_classifies_validated_artifacts() {
        let fixture = Fixture::new();
        fixture.rust("rust-app");
        fixture.node("node-app");
        let report = fixture.scan_old();
        assert_eq!(report.candidates.len(), 2);
        assert!(report.candidates.iter().all(|candidate| {
            candidate.path.is_absolute()
                && candidate.project_root.is_absolute()
                && !candidate.evidence.is_empty()
                && candidate.bytes > 0
                && candidate.files > 0
        }));
        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| candidate.kind == ArtifactKind::RustTarget)
        );
        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| candidate.kind == ArtifactKind::NodeModules)
        );
    }

    #[test]
    fn cleaner_cli_expands_current_user_home_only() {
        let fixture = Fixture::new();
        assert_eq!(
            expand_cli_root(&fixture.layout, "~/Work/project").unwrap(),
            fixture.layout.home.join("Work/project")
        );
        assert_eq!(
            expand_cli_root(&fixture.layout, "/tmp/project").unwrap(),
            PathBuf::from("/tmp/project")
        );
        assert!(expand_cli_root(&fixture.layout, "~someone/Work").is_err());
    }

    #[test]
    fn cleaner_scan_excludes_ambiguous_and_overlapping_paths() {
        let fixture = Fixture::new();
        let node = fixture.node("node-app");
        fs::create_dir_all(fixture.work.join("notes/target")).unwrap();
        fs::create_dir_all(fixture.work.join("notes/build")).unwrap();
        fs::create_dir_all(fixture.work.join("notes/dist")).unwrap();
        let report = fixture.scan_old();
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].path, node.canonicalize().unwrap());
        assert_eq!(
            report.total_bytes,
            b"node-bytes".len() as u64 + b"vite".len() as u64
        );
    }

    #[test]
    fn cleaner_scan_does_not_follow_symlinks_or_git() {
        let fixture = Fixture::new();
        let outside = fixture.layout.home.join("outside");
        fs::create_dir_all(outside.join("target/debug")).unwrap();
        fs::write(outside.join("Cargo.toml"), "[workspace]").unwrap();
        fs::write(outside.join("target/.rustc_info.json"), "{}").unwrap();
        symlink(&outside, fixture.work.join("outside-link")).unwrap();
        let git = fixture.work.join("repo/.git/hidden");
        fs::create_dir_all(git.join("target/debug")).unwrap();
        fs::write(git.join("Cargo.toml"), "[workspace]").unwrap();
        fs::write(git.join("target/.rustc_info.json"), "{}").unwrap();
        let report = fixture.scan_old();
        assert!(report.candidates.is_empty());
        assert!(report.skipped_boundaries >= 2);
    }

    #[test]
    fn cleaner_rejects_unsafe_roots() {
        let fixture = Fixture::new();
        let file = fixture.layout.home.join("file");
        fs::write(&file, "x").unwrap();
        for root in [
            PathBuf::from("relative"),
            PathBuf::from("/"),
            fixture.layout.home.clone(),
            file,
            fixture.work.join("missing"),
        ] {
            assert!(
                scan(&fixture.layout, &root).is_err(),
                "accepted {}",
                root.display()
            );
        }
    }

    #[test]
    fn cleaner_scan_and_plan_are_read_only() {
        let fixture = Fixture::new();
        let target = fixture.rust("app");
        let before = fs::read(target.join("debug/app")).unwrap();
        let report = fixture.scan_old();
        let plan = create_plan_at(
            &fixture.layout,
            &fixture.work,
            &[report.candidates[0].id.clone()],
            Fixture::old_now(),
            RECENT_WRITE_GUARD,
        )
        .unwrap();
        assert_eq!(fs::read(target.join("debug/app")).unwrap(), before);
        assert!(fixture.layout.plan_path(&plan.id).unwrap().is_file());
    }

    #[test]
    fn cleaner_apply_removes_only_confirmed_candidates() {
        let fixture = Fixture::new();
        let selected = fixture.rust("selected");
        let retained = fixture.rust("retained");
        let report = fixture.scan_old();
        let id = report
            .candidates
            .iter()
            .find(|candidate| candidate.path == selected.canonicalize().unwrap())
            .unwrap()
            .id
            .clone();
        let now = Fixture::old_now();
        let plan = create_plan_at(
            &fixture.layout,
            &fixture.work,
            &[id],
            now,
            RECENT_WRITE_GUARD,
        )
        .unwrap();
        let applied =
            apply_plan_at(&fixture.layout, &plan.id, &plan.confirmation_token, now).unwrap();
        assert_eq!(applied.removed, 1);
        assert!(applied.reclaimed_bytes > 0);
        assert!(!selected.exists());
        assert!(retained.exists());
    }

    #[test]
    fn cleaner_apply_rejects_missing_or_wrong_confirmation() {
        let fixture = Fixture::new();
        let target = fixture.rust("app");
        let report = fixture.scan_old();
        let plan = create_plan_at(
            &fixture.layout,
            &fixture.work,
            &[report.candidates[0].id.clone()],
            Fixture::old_now(),
            RECENT_WRITE_GUARD,
        )
        .unwrap();
        assert!(apply_plan(&fixture.layout, &plan.id, "").is_err());
        assert!(apply_plan(&fixture.layout, &plan.id, "wrong").is_err());
        assert!(target.exists());
    }

    #[test]
    fn cleaner_apply_skips_replaced_candidate() {
        let fixture = Fixture::new();
        let target = fixture.rust("app");
        let report = fixture.scan_old();
        let now = Fixture::old_now();
        let plan = create_plan_at(
            &fixture.layout,
            &fixture.work,
            &[report.candidates[0].id.clone()],
            now,
            RECENT_WRITE_GUARD,
        )
        .unwrap();
        fs::remove_dir_all(&target).unwrap();
        fs::create_dir_all(&target).unwrap();
        let applied =
            apply_plan_at(&fixture.layout, &plan.id, &plan.confirmation_token, now).unwrap();
        assert_eq!(
            applied.items[0].reason.as_deref(),
            Some("identity-mismatch")
        );
        assert!(target.exists());
    }

    #[test]
    fn cleaner_apply_skips_recent_candidate() {
        let fixture = Fixture::new();
        let target = fixture.rust("app");
        let report = fixture.scan_old();
        let now = Fixture::old_now();
        let plan = create_plan_at(
            &fixture.layout,
            &fixture.work,
            &[report.candidates[0].id.clone()],
            now,
            RECENT_WRITE_GUARD,
        )
        .unwrap();
        fs::write(target.join("new-object"), "new").unwrap();
        let applied =
            apply_plan_at(&fixture.layout, &plan.id, &plan.confirmation_token, now).unwrap();
        assert_eq!(applied.items[0].reason.as_deref(), Some("recent-write"));
        assert!(target.exists());
    }

    #[test]
    fn cleaner_plugin_install_is_user_owned() {
        let fixture = Fixture::new();
        let destination = install_plugin(&fixture.layout).unwrap();
        assert!(destination.starts_with(&fixture.layout.config_root));
        assert_eq!(
            fs::read_to_string(destination.join("manifest.json")).unwrap(),
            PLUGIN_MANIFEST
        );
        assert_eq!(
            fs::read_to_string(destination.join("Panel.qml")).unwrap(),
            PLUGIN_PANEL
        );
        assert!(destination.join(".omarchy-rs-owner.json").is_file());
        uninstall_plugin(&fixture.layout).unwrap();
        assert!(!destination.exists());
    }

    #[test]
    fn cleaner_plugin_refuses_foreign_files() {
        let fixture = Fixture::new();
        let destination = fixture
            .layout
            .config_root
            .join("omarchy/plugins")
            .join(PLUGIN_ID);
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("foreign"), b"keep-me").unwrap();
        assert!(install_plugin(&fixture.layout).is_err());
        assert!(uninstall_plugin(&fixture.layout).is_err());
        assert_eq!(fs::read(destination.join("foreign")).unwrap(), b"keep-me");
    }

    #[test]
    fn cleaner_plugin_uses_rust_json_commands() {
        for command in ["scan", "plan", "apply"] {
            assert!(PLUGIN_PANEL.contains(&format!("\"cleaner\", \"{command}\"")));
        }
        for forbidden in ["bash", "sh\"", "sudo", "pkexec", "pacman"] {
            assert!(!PLUGIN_PANEL.contains(forbidden), "found {forbidden}");
        }
    }

    fn benchmark_report() -> CleanerBenchmarkReport {
        CleanerBenchmarkReport {
            schema_version: SCHEMA_VERSION,
            config: CleanerBenchmarkConfig {
                fixture_version: "workspace-v1".into(),
                projects: 12,
                files_per_artifact: 1000,
                bytes_per_file: 128,
                warmups: 3,
                samples: 30,
            },
            python: CleanerBenchmarkMetrics {
                median_wall_ms: 100.0,
                median_cpu_ms: 90.0,
                max_rss_kib: 10_000,
                median_read_bytes: Some(4096),
                median_written_bytes: Some(0),
                child_process_count: 0,
            },
            rust: CleanerBenchmarkMetrics {
                median_wall_ms: 50.0,
                median_cpu_ms: 45.0,
                max_rss_kib: 6_000,
                median_read_bytes: Some(4096),
                median_written_bytes: Some(0),
                child_process_count: 0,
            },
        }
    }

    #[test]
    fn cleaner_benchmark_report_has_comparable_metrics() {
        let report = benchmark_report();
        validate_benchmark_report(&report).unwrap();
        assert_eq!(report.config.warmups, 3);
        assert_eq!(report.config.samples, 30);
        assert!(report.python.median_read_bytes.is_some());
        assert!(report.rust.median_read_bytes.is_some());
    }

    #[test]
    fn cleaner_benchmark_gate_rejects_regression() {
        let mut report = benchmark_report();
        report.rust.median_wall_ms = 95.0;
        report.rust.max_rss_kib = 11_500;
        let eligibility = benchmark_eligibility(&report);
        assert!(!eligibility.default_enabled);
        assert!(
            eligibility
                .reasons
                .iter()
                .any(|reason| reason.contains("40%"))
        );
        assert!(
            eligibility
                .reasons
                .iter()
                .any(|reason| reason.contains("regression"))
        );
    }
}
