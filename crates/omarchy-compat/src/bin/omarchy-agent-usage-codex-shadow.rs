use std::{env, path::PathBuf, process::Command};

use omarchy_compat::shadow::{
    compare_local_fields, sanitized_receipt, validate_record, verified_absolute_executable,
};

const DEFAULT_UPSTREAM: &str = "/usr/share/omarchy/bin/omarchy-agent-usage-codex";

fn main() -> Result<(), String> {
    let upstream = env::var_os("OMARCHY_RS_CODEX_UPSTREAM")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_UPSTREAM));
    verified_absolute_executable(&upstream)?;
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let output = Command::new(&upstream)
        .args(&args)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        std::process::exit(output.status.code().unwrap_or(1));
    }
    let upstream_record = validate_record(&output.stdout)?;

    let candidate = env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .ok_or_else(|| "HOME and CODEX_HOME are unset".to_string())
        .and_then(|codex_home| {
            omarchy_agents::codex::collect_local_record(&codex_home.join("sessions"))
        });
    match candidate {
        Ok(candidate) => {
            let comparison = compare_local_fields(&candidate, &upstream_record);
            eprintln!("omarchy-rs-shadow {}", sanitized_receipt(&comparison));
        }
        Err(_) => eprintln!("omarchy-rs-shadow candidate-error"),
    }

    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}
