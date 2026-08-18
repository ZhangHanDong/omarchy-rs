use std::{env, fs, path::PathBuf, process::Command};

use omarchy_compat::activation::CLAUDE_UPSTREAM_SHA256;
use omarchy_compat::shadow::{
    claude_canary_eligible, validate_provider_record, verified_absolute_executable,
};
use sha2::{Digest, Sha256};

const DEFAULT_UPSTREAM: &str = "/usr/share/omarchy/bin/omarchy-agent-usage-claude";

fn main() -> Result<(), String> {
    let upstream = env::var_os("OMARCHY_RS_CLAUDE_UPSTREAM")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_UPSTREAM));
    verified_absolute_executable(&upstream)?;
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let config_dir = env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude")))
        .ok_or("HOME and CLAUDE_CONFIG_DIR are unset")?;
    let force = args.iter().any(|arg| arg == "--force");
    let candidate = omarchy_agents::claude::CollectOptions::from_environment(force)
        .and_then(|options| omarchy_agents::claude::collect_record(&config_dir, &options));

    if env::var("OMARCHY_RS_CLAUDE_MODE").as_deref() == Ok("canary")
        && let Ok(record) = &candidate
        && claude_canary_eligible(
            upstream_fingerprint(&upstream).as_deref() == Ok(CLAUDE_UPSTREAM_SHA256),
            has_external_sources(),
            args.iter().any(|arg| arg == "--limits-only"),
            record,
        )
    {
        println!(
            "{}",
            serde_json::to_string(record).map_err(|error| error.to_string())?
        );
        return Ok(());
    }

    let output = Command::new(&upstream)
        .args(&args)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        std::process::exit(output.status.code().unwrap_or(1));
    }
    let mut record = validate_provider_record(&output.stdout, "claude")?;
    record
        .as_object_mut()
        .ok_or("upstream record is not an object")?
        .insert("collectorBackend".into(), "python".into());
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    println!(
        "{}",
        serde_json::to_string(&record).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn upstream_fingerprint(path: &std::path::Path) -> Result<String, String> {
    fs::read(path)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|error| error.to_string())
}

fn has_external_sources() -> bool {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return true;
    };
    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    home.join(".pi/agent/sessions").exists()
        || home.join(".omp/agent/sessions").exists()
        || data_home.join("opencode/opencode.db").exists()
}
