use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use chrono::Utc;
use omarchy_agents::benchmark::{
    AbBenchmarkReport, BenchmarkEnvironment, BenchmarkReport, BenchmarkSample,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

static SHOWED_NORMALIZED_OUTPUT: AtomicBool = AtomicBool::new(false);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--ab") {
        return run_ab(&args);
    }
    let upstream = args
        .get(1)
        .ok_or("usage: codex-benchmark UPSTREAM [SAMPLES] [SESSIONS]")?;
    let measured_samples = args.get(2).map_or(Ok(100), |v| v.parse())?;
    let fixture_sessions = args.get(3).map_or(Ok(100), |v| v.parse())?;
    let implementation = args
        .get(4)
        .cloned()
        .unwrap_or_else(|| "omarchy-upstream".into());
    let output_path = args.get(5);
    let fixture = args.get(6).map_or("valid", String::as_str);
    let warmups = 10;
    let clock_ticks = clock_ticks_per_second()?;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = workspace.join(format!(
        "fixtures/agent_usage/codex/{fixture}/session.jsonl"
    ));
    let isolated = TempDir::new()?;
    let home = isolated.path().join("home");
    let sessions = home.join(".codex/sessions");
    let cache = isolated.path().join("cache");
    let data = isolated.path().join("data");
    fs::create_dir_all(&sessions)?;
    fs::create_dir_all(&cache)?;
    fs::create_dir_all(&data)?;
    for index in 0..fixture_sessions {
        fs::copy(&source, sessions.join(format!("session-{index}.jsonl")))?;
    }

    for _ in 0..warmups {
        run_once(upstream, &home, &cache, &data, clock_ticks)?;
    }
    let mut samples = Vec::with_capacity(measured_samples);
    for _ in 0..measured_samples {
        samples.push(run_once(upstream, &home, &cache, &data, clock_ticks)?);
    }
    let report = BenchmarkReport {
        schema_version: 1,
        implementation,
        executable: upstream.into(),
        fixture: fixture.into(),
        fixture_sessions,
        warmups,
        measured_samples,
        logical_cache_state: "forced-rescan".into(),
        samples,
    };
    report.validate()?;
    let encoded = serde_json::to_string_pretty(&report)? + "\n";
    if let Some(path) = output_path {
        fs::write(path, &encoded)?;
    }
    print!("{encoded}");
    Ok(())
}

fn run_ab(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let upstream = args
        .get(2)
        .ok_or("usage: codex-benchmark --ab UPSTREAM CANDIDATE [SAMPLES] [SESSIONS] [OUTPUT]")?;
    let candidate = args
        .get(3)
        .ok_or("usage: codex-benchmark --ab UPSTREAM CANDIDATE [SAMPLES] [SESSIONS] [OUTPUT]")?;
    let measured_samples = args.get(4).map_or(Ok(100), |value| value.parse())?;
    let fixture_sessions = args.get(5).map_or(Ok(100), |value| value.parse())?;
    let output_path = args.get(6);
    let fixture = args.get(7).map_or("valid", String::as_str);
    let warmups = 10;
    let clock_ticks = clock_ticks_per_second()?;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = workspace.join(format!(
        "fixtures/agent_usage/codex/{fixture}/session.jsonl"
    ));
    let isolated = TempDir::new()?;
    let home = isolated.path().join("home");
    let sessions = home.join(".codex/sessions");
    let cache = isolated.path().join("cache");
    let data = isolated.path().join("data");
    fs::create_dir_all(&sessions)?;
    fs::create_dir_all(&cache)?;
    fs::create_dir_all(&data)?;
    for index in 0..fixture_sessions {
        fs::copy(&source, sessions.join(format!("session-{index}.jsonl")))?;
    }

    for index in 0..warmups {
        let (first, second) = if index % 2 == 0 {
            (upstream, candidate)
        } else {
            (candidate, upstream)
        };
        run_once(first, &home, &cache, &data, clock_ticks)?;
        run_once(second, &home, &cache, &data, clock_ticks)?;
    }

    let mut upstream_samples = Vec::with_capacity(measured_samples);
    let mut candidate_samples = Vec::with_capacity(measured_samples);
    for index in 0..measured_samples {
        if index % 2 == 0 {
            upstream_samples.push(run_once(upstream, &home, &cache, &data, clock_ticks)?);
            candidate_samples.push(run_once(candidate, &home, &cache, &data, clock_ticks)?);
        } else {
            candidate_samples.push(run_once(candidate, &home, &cache, &data, clock_ticks)?);
            upstream_samples.push(run_once(upstream, &home, &cache, &data, clock_ticks)?);
        }
    }

    let upstream_report = implementation_report(
        "omarchy-upstream",
        upstream,
        fixture_sessions,
        warmups,
        fixture,
        upstream_samples,
    );
    let candidate_report = implementation_report(
        "omarchy-rs",
        candidate,
        fixture_sessions,
        warmups,
        fixture,
        candidate_samples,
    );
    let upstream_hashes = output_hashes(&upstream_report);
    let candidate_hashes = output_hashes(&candidate_report);
    let report = AbBenchmarkReport {
        schema_version: 1,
        environment: benchmark_environment(upstream, candidate, clock_ticks, &workspace)?,
        execution_order: "alternating AB/BA pairs".into(),
        output_hashes_match: upstream_hashes.len() == 1
            && candidate_hashes.len() == 1
            && upstream_hashes == candidate_hashes,
        upstream: upstream_report,
        candidate: candidate_report,
    };
    report.validate()?;
    let encoded = serde_json::to_string_pretty(&report)? + "\n";
    if let Some(path) = output_path {
        fs::write(path, &encoded)?;
    }
    print!("{encoded}");
    Ok(())
}

fn implementation_report(
    implementation: &str,
    executable: &str,
    fixture_sessions: usize,
    warmups: usize,
    fixture: &str,
    samples: Vec<BenchmarkSample>,
) -> BenchmarkReport {
    BenchmarkReport {
        schema_version: 1,
        implementation: implementation.into(),
        executable: executable.into(),
        fixture: fixture.into(),
        fixture_sessions,
        warmups,
        measured_samples: samples.len(),
        logical_cache_state: "forced-rescan".into(),
        samples,
    }
}

fn output_hashes(report: &BenchmarkReport) -> std::collections::BTreeSet<&str> {
    report
        .samples
        .iter()
        .map(|sample| sample.normalized_output_sha256.as_str())
        .collect()
}

fn benchmark_environment(
    upstream: &str,
    candidate: &str,
    clock_ticks: f64,
    workspace: &Path,
) -> Result<BenchmarkEnvironment, Box<dyn std::error::Error>> {
    let os_release = fs::read_to_string("/etc/os-release")?
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .unwrap_or("unknown")
        .trim_matches('"')
        .to_string();
    let cpu_model = fs::read_to_string("/proc/cpuinfo")?
        .lines()
        .find_map(|line| line.strip_prefix("model name\t: "))
        .unwrap_or("unknown")
        .to_string();
    Ok(BenchmarkEnvironment {
        captured_at_utc: Utc::now().to_rfc3339(),
        os_release,
        kernel_release: command_text("uname", &["-r"], None)?,
        cpu_model,
        clock_ticks_per_second: clock_ticks as u64,
        upstream_sha256: file_sha256(upstream)?,
        candidate_sha256: file_sha256(candidate)?,
        omarchy_rs_commit: command_text("git", &["rev-parse", "HEAD"], Some(workspace))?,
        ccusage_commit: "97f5b4e71864408c4df5a9758639d253caf57dce".into(),
    })
}

fn file_sha256(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

fn command_text(
    executable: &str,
    args: &[&str],
    current_dir: Option<&Path>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut command = Command::new(executable);
    command.args(args);
    if let Some(path) = current_dir {
        command.current_dir(path);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!("{executable} failed").into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn run_once(
    executable: &str,
    home: &Path,
    cache: &Path,
    data: &Path,
    clock_ticks: f64,
) -> Result<BenchmarkSample, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut child = Command::new(executable)
        .arg("--force")
        .env_clear()
        .env("HOME", home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("XDG_CACHE_HOME", cache)
        .env("XDG_DATA_HOME", data)
        .env("PATH", "/usr/bin:/bin")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let proc_root = PathBuf::from(format!("/proc/{}", child.id()));
    let mut usage = ProcUsage::default();
    loop {
        usage.observe(&proc_root);
        if child.try_wait()?.is_some() {
            break;
        }
        thread::sleep(Duration::from_micros(100));
    }
    let wall_ns = started.elapsed().as_nanos();
    let status = child.wait()?;
    let mut stdout = Vec::new();
    child
        .stdout
        .take()
        .ok_or("missing stdout")?
        .read_to_end(&mut stdout)?;
    let mut stderr = Vec::new();
    child
        .stderr
        .take()
        .ok_or("missing stderr")?
        .read_to_end(&mut stderr)?;
    if !status.success() {
        return Err(format!("collector failed: {}", String::from_utf8_lossy(&stderr)).into());
    }
    let mut json: Value = serde_json::from_slice(&stdout)?;
    json.as_object_mut()
        .ok_or("collector output is not an object")?
        .remove("updatedAt");
    let normalized = serde_json::to_vec(&json)?;
    if env::var_os("OMARCHY_BENCHMARK_SHOW_OUTPUT").is_some()
        && !SHOWED_NORMALIZED_OUTPUT.swap(true, Ordering::Relaxed)
    {
        eprintln!("{}", serde_json::to_string_pretty(&json)?);
    }
    Ok(BenchmarkSample {
        wall_ns,
        user_seconds: usage.user_ticks as f64 / clock_ticks,
        system_seconds: usage.system_ticks as f64 / clock_ticks,
        max_rss_kib: usage.max_rss_kib,
        read_bytes: usage.read_bytes,
        write_bytes: usage.write_bytes,
        voluntary_context_switches: usage.voluntary_context_switches,
        involuntary_context_switches: usage.involuntary_context_switches,
        child_process_count: usage.max_children,
        exit_code: status.code().unwrap_or(-1),
        normalized_output_sha256: format!("{:x}", Sha256::digest(normalized)),
    })
}

#[derive(Default)]
struct ProcUsage {
    user_ticks: u64,
    system_ticks: u64,
    max_rss_kib: u64,
    read_bytes: u64,
    write_bytes: u64,
    voluntary_context_switches: u64,
    involuntary_context_switches: u64,
    max_children: u64,
}

impl ProcUsage {
    fn observe(&mut self, root: &Path) {
        if let Ok(stat) = fs::read_to_string(root.join("stat"))
            && let Some(fields) = stat.rsplit_once(") ").map(|(_, rest)| rest)
        {
            let fields: Vec<&str> = fields.split_whitespace().collect();
            self.user_ticks = fields
                .get(11)
                .and_then(|v| v.parse().ok())
                .unwrap_or(self.user_ticks);
            self.system_ticks = fields
                .get(12)
                .and_then(|v| v.parse().ok())
                .unwrap_or(self.system_ticks);
        }
        if let Ok(status) = fs::read_to_string(root.join("status")) {
            for line in status.lines() {
                update_named(line, "VmHWM:", &mut self.max_rss_kib);
                update_named(
                    line,
                    "voluntary_ctxt_switches:",
                    &mut self.voluntary_context_switches,
                );
                update_named(
                    line,
                    "nonvoluntary_ctxt_switches:",
                    &mut self.involuntary_context_switches,
                );
            }
        }
        if let Ok(io) = fs::read_to_string(root.join("io")) {
            for line in io.lines() {
                update_named(line, "read_bytes:", &mut self.read_bytes);
                update_named(line, "write_bytes:", &mut self.write_bytes);
            }
        }
        if let Ok(children) = fs::read_to_string(
            root.join("task")
                .join(root.file_name().unwrap())
                .join("children"),
        ) {
            self.max_children = self
                .max_children
                .max(children.split_whitespace().count() as u64);
        }
    }
}

fn update_named(line: &str, name: &str, target: &mut u64) {
    if let Some(value) = line
        .strip_prefix(name)
        .and_then(|v| v.split_whitespace().next())
        .and_then(|v| v.parse().ok())
    {
        *target = (*target).max(value);
    }
}

fn clock_ticks_per_second() -> Result<f64, Box<dyn std::error::Error>> {
    let output = Command::new("getconf").arg("CLK_TCK").output()?;
    Ok(String::from_utf8(output.stdout)?.trim().parse()?)
}
