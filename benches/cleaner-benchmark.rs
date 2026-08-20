use omarchy_rs::cleaner::{
    CleanerBenchmarkConfig, CleanerBenchmarkMetrics, CleanerBenchmarkReport, SCHEMA_VERSION,
    benchmark_eligibility,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const PROJECTS: u32 = 12;
const FILES_PER_ARTIFACT: u32 = 1000;
const BYTES_PER_FILE: u32 = 128;
const WARMUPS: u32 = 3;
const SAMPLES: u32 = 30;

#[derive(Clone, Copy)]
struct Sample {
    wall_ms: f64,
    cpu_ms: f64,
    rss_kib: u64,
    read_bytes: u64,
    written_bytes: u64,
    child_process_count: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rust = manifest.join("target/release/omarchy-rs");
    if !rust.is_file() {
        return Err("build target/release/omarchy-rs before running the benchmark".into());
    }
    let python = manifest.join("benches/cleaner_reference.py");
    let fixture = tempfile::tempdir()?;
    let home = fixture.path().join("home");
    let work = home.join("Work");
    generate_fixture(&work)?;

    let python_command = vec![
        "python3".into(),
        python.to_string_lossy().into_owned(),
        work.to_string_lossy().into_owned(),
    ];
    let rust_command = vec![
        rust.to_string_lossy().into_owned(),
        "cleaner".into(),
        "scan".into(),
        "--root".into(),
        work.to_string_lossy().into_owned(),
        "--json".into(),
    ];
    let python_metrics = measure(&python_command, &home, fixture.path())?;
    let rust_metrics = measure(&rust_command, &home, fixture.path())?;
    let report = CleanerBenchmarkReport {
        schema_version: SCHEMA_VERSION,
        config: CleanerBenchmarkConfig {
            fixture_version: "workspace-v1".into(),
            projects: PROJECTS,
            files_per_artifact: FILES_PER_ARTIFACT,
            bytes_per_file: BYTES_PER_FILE,
            warmups: WARMUPS,
            samples: SAMPLES,
        },
        python: python_metrics,
        rust: rust_metrics,
    };
    let eligibility = benchmark_eligibility(&report);
    let output = serde_json::json!({
        "report": report,
        "eligibility": eligibility,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    if !eligibility.default_enabled {
        return Err("Rust cleaner did not satisfy the benchmark gate".into());
    }
    Ok(())
}

fn generate_fixture(work: &Path) -> std::io::Result<()> {
    let bytes = vec![b'x'; BYTES_PER_FILE as usize];
    for index in 0..PROJECTS {
        let rust = work.join(format!("rust-{index}/target/debug/deps"));
        fs::create_dir_all(&rust)?;
        fs::write(
            work.join(format!("rust-{index}/Cargo.toml")),
            "[workspace]\n",
        )?;
        fs::write(
            work.join(format!("rust-{index}/target/.rustc_info.json")),
            "{}",
        )?;
        let node = work.join(format!("node-{index}/node_modules/pkg"));
        fs::create_dir_all(&node)?;
        fs::write(work.join(format!("node-{index}/package.json")), "{}")?;
        for file in 0..FILES_PER_ARTIFACT {
            fs::write(rust.join(format!("artifact-{file}.o")), &bytes)?;
            fs::write(node.join(format!("module-{file}.js")), &bytes)?;
        }
    }
    Ok(())
}

fn measure(
    command: &[String],
    home: &Path,
    scratch: &Path,
) -> Result<CleanerBenchmarkMetrics, Box<dyn std::error::Error>> {
    let clock_ticks = String::from_utf8(Command::new("getconf").arg("CLK_TCK").output()?.stdout)?
        .trim()
        .parse::<f64>()?;
    for _ in 0..WARMUPS {
        run_once(command, home, scratch, clock_ticks)?;
    }
    let mut samples = Vec::new();
    for _ in 0..SAMPLES {
        samples.push(run_once(command, home, scratch, clock_ticks)?);
    }
    let mut wall = samples
        .iter()
        .map(|sample| sample.wall_ms)
        .collect::<Vec<_>>();
    let mut cpu = samples
        .iter()
        .map(|sample| sample.cpu_ms)
        .collect::<Vec<_>>();
    let mut reads = samples
        .iter()
        .map(|sample| sample.read_bytes)
        .collect::<Vec<_>>();
    let mut writes = samples
        .iter()
        .map(|sample| sample.written_bytes)
        .collect::<Vec<_>>();
    wall.sort_by(f64::total_cmp);
    cpu.sort_by(f64::total_cmp);
    reads.sort_unstable();
    writes.sort_unstable();
    Ok(CleanerBenchmarkMetrics {
        median_wall_ms: median(&wall),
        median_cpu_ms: median(&cpu),
        max_rss_kib: samples
            .iter()
            .map(|sample| sample.rss_kib)
            .max()
            .unwrap_or(0),
        median_read_bytes: Some(reads[reads.len() / 2]),
        median_written_bytes: Some(writes[writes.len() / 2]),
        child_process_count: samples
            .iter()
            .map(|sample| sample.child_process_count)
            .max()
            .unwrap_or(0),
    })
}

fn run_once(
    command: &[String],
    home: &Path,
    _scratch: &Path,
    clock_ticks: f64,
) -> Result<Sample, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .env("HOME", home)
        .env("XDG_STATE_HOME", home.join(".local/state"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let pid = child.id();
    let mut peak_rss_kib = 0;
    let mut cpu_ticks = 0;
    let mut read_bytes = 0;
    let mut written_bytes = 0;
    let mut child_process_count = 0;
    let status = loop {
        if let Some(sample) = proc_sample(pid) {
            peak_rss_kib = peak_rss_kib.max(sample.0);
            cpu_ticks = cpu_ticks.max(sample.1);
            read_bytes = read_bytes.max(sample.2);
            written_bytes = written_bytes.max(sample.3);
            child_process_count = child_process_count.max(sample.4);
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_micros(500));
    };
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    if !status.success() {
        return Err(format!("benchmark command failed: {}", command.join(" ")).into());
    }
    Ok(Sample {
        wall_ms,
        cpu_ms: cpu_ticks as f64 * 1000.0 / clock_ticks,
        rss_kib: peak_rss_kib,
        read_bytes,
        written_bytes,
        child_process_count,
    })
}

fn proc_sample(pid: u32) -> Option<(u64, u64, u64, u64, u32)> {
    let base = PathBuf::from(format!("/proc/{pid}"));
    let status = fs::read_to_string(base.join("status")).ok()?;
    let rss = status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let stat = fs::read_to_string(base.join("stat")).ok()?;
    let fields = stat
        .split_once(") ")?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let cpu_ticks = fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
    let io = fs::read_to_string(base.join("io")).unwrap_or_default();
    let read_bytes = proc_value(&io, "read_bytes:");
    let written_bytes = proc_value(&io, "write_bytes:");
    let children =
        fs::read_to_string(base.join(format!("task/{pid}/children"))).unwrap_or_default();
    let child_process_count = children.split_whitespace().count() as u32;
    Some((
        rss,
        cpu_ticks,
        read_bytes,
        written_bytes,
        child_process_count,
    ))
}

fn proc_value(input: &str, key: &str) -> u64 {
    input
        .lines()
        .find(|line| line.starts_with(key))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn median(values: &[f64]) -> f64 {
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}
