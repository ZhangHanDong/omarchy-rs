use std::{env, fs, path::PathBuf, process::Command};

use omarchy_compat::shadow::{
    canary_eligible, compare_local_fields, sanitized_receipt, validate_record,
    verified_absolute_executable,
};
use sha2::{Digest, Sha256};

const DEFAULT_UPSTREAM: &str = "/usr/share/omarchy/bin/omarchy-agent-usage-codex";
const VERIFIED_UPSTREAM_SHA256: &str =
    "0d36d856439f17749dc8a25c56607e8462de72fde91f384abc370fbc78113b14";

fn main() -> Result<(), String> {
    let upstream = env::var_os("OMARCHY_RS_CODEX_UPSTREAM")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_UPSTREAM));
    verified_absolute_executable(&upstream)?;
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let candidate = env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .ok_or_else(|| "HOME and CODEX_HOME are unset".to_string())
        .and_then(|codex_home| omarchy_agents::codex::collect_record(&codex_home.join("sessions")));

    if env::var("OMARCHY_RS_CODEX_MODE").as_deref() == Ok("canary")
        && let Ok(record) = &candidate
        && canary_eligible(
            upstream_fingerprint(&upstream).as_deref() == Ok(VERIFIED_UPSTREAM_SHA256),
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
    let mut upstream_record = validate_record(&output.stdout)?;
    upstream_record
        .as_object_mut()
        .ok_or("upstream record is not an object")?
        .insert("collectorBackend".into(), "python".into());

    match candidate {
        Ok(candidate) => {
            let comparison = compare_local_fields(&candidate, &upstream_record);
            eprintln!("omarchy-rs-shadow {}", sanitized_receipt(&comparison));
        }
        Err(_) => eprintln!("omarchy-rs-shadow candidate-error"),
    }

    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    println!(
        "{}",
        serde_json::to_string(&upstream_record).map_err(|error| error.to_string())?
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
