use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use omarchy_compat::shadow::{
    octoscode_canary_eligible, validate_provider_record, verified_absolute_executable,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const VERIFIED_SHA256: &str = "d67554a97fd4c27bec3c1557f06fba4498aaebe949eb8836d7c145ce9a9b707a";

fn main() -> Result<(), String> {
    let upstream = env::var_os("OMARCHY_RS_OCTOSCODE_UPSTREAM")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".local/bin/omarchy-agent-usage-octoscode"))
        })
        .ok_or("HOME and OMARCHY_RS_OCTOSCODE_UPSTREAM are unset")?;
    verified_absolute_executable(&upstream)?;
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg != "--write") {
        return fallback(&upstream, &args);
    }
    let home = env::var_os("OCTOS_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".octos")))
        .ok_or("HOME and OCTOS_HOME are unset")?;
    let candidate = omarchy_agents::octoscode::collect_record(&home);
    let record = if env::var("OMARCHY_RS_OCTOSCODE_MODE").as_deref() == Ok("canary")
        && octoscode_canary_eligible(
            fingerprint(&upstream).as_deref() == Ok(VERIFIED_SHA256),
            &candidate,
        ) {
        candidate
    } else {
        upstream_record(&upstream)?
    };
    if args.iter().any(|arg| arg == "--write") {
        write_state(&record)?;
    } else {
        println!(
            "{}",
            serde_json::to_string(&record).map_err(|error| error.to_string())?
        );
    }
    Ok(())
}

fn upstream_record(upstream: &Path) -> Result<Value, String> {
    let output = Command::new(upstream)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    let mut record = validate_provider_record(&output.stdout, "octoscode")?;
    record
        .as_object_mut()
        .ok_or("record is not an object")?
        .insert("collectorBackend".into(), "python".into());
    Ok(record)
}

fn fallback(upstream: &Path, args: &[String]) -> Result<(), String> {
    let status = Command::new(upstream)
        .args(args)
        .status()
        .map_err(|error| error.to_string())?;
    std::process::exit(status.code().unwrap_or(1));
}

fn write_state(record: &Value) -> Result<(), String> {
    let state = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .ok_or("HOME and XDG_STATE_HOME are unset")?;
    let directory = state.join("omarchy/agents/usage");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let temporary = directory.join(format!(".octoscode.omarchy-rs.{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(&serde_json::to_vec(record).map_err(std::io::Error::other)?)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, directory.join("octoscode.json"))
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
    #[test]
    fn octoscode_state_write_is_atomic() {
        let state = tempfile::tempdir().unwrap();
        unsafe {
            env::set_var("XDG_STATE_HOME", state.path());
        }
        let record = serde_json::json!({"schemaVersion":1,"id":"octoscode","name":"Octoscode","ready":true,"limits":[],"collectorBackend":"rust"});
        write_state(&record).unwrap();
        let bytes = fs::read(state.path().join("omarchy/agents/usage/octoscode.json")).unwrap();
        assert_eq!(
            validate_provider_record(&bytes, "octoscode").unwrap()["collectorBackend"],
            "rust"
        );
        unsafe {
            env::remove_var("XDG_STATE_HOME");
        }
    }
}
