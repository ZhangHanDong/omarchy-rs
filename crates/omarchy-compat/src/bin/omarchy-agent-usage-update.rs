use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

const DEFAULT_UPSTREAM_UPDATE: &str = "/usr/share/omarchy/bin/omarchy-agent-usage-update";

fn main() -> Result<ExitCode, String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let upstream = env::var_os("OMARCHY_RS_UPDATE_UPSTREAM")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_UPSTREAM_UPDATE));
    require_absolute_file(&upstream)?;
    if !codex_is_wanted(&args)? {
        return command_status(&upstream, &args);
    }

    let mut upstream_args = args.clone();
    upstream_args.extend(["--except".into(), "codex".into()]);
    let other_status = command_status(&upstream, &upstream_args)?;

    let shadow = env::var_os("OMARCHY_RS_CODEX_SHADOW")
        .map(PathBuf::from)
        .unwrap_or(current_sibling("omarchy-agent-usage-codex-shadow")?);
    require_absolute_file(&shadow)?;
    let flags = args
        .iter()
        .filter(|arg| matches!(arg.as_str(), "--force" | "--limits-only"));
    let output = Command::new(shadow)
        .args(flags)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
    if !output.status.success() {
        return Ok(ExitCode::FAILURE);
    }
    omarchy_compat::shadow::validate_record(&output.stdout)?;
    write_codex_state(&output.stdout)?;

    Ok(if other_status == ExitCode::SUCCESS {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn codex_is_wanted(args: &[String]) -> Result<bool, String> {
    let mut only = Vec::new();
    let mut excluded = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--force" | "--limits-only" => {}
            "--except" => {
                index += 1;
                excluded.push(
                    args.get(index)
                        .ok_or("--except requires an agent")?
                        .as_str(),
                );
            }
            value if value.starts_with('-') => return Err(format!("unknown option: {value}")),
            value => only.push(value),
        }
        index += 1;
    }
    Ok(!excluded.contains(&"codex") && (only.is_empty() || only.contains(&"codex")))
}

fn command_status(path: &Path, args: &[String]) -> Result<ExitCode, String> {
    let status = Command::new(path)
        .args(args)
        .status()
        .map_err(|error| error.to_string())?;
    Ok(if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn write_codex_state(record: &[u8]) -> Result<(), String> {
    let state_home = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .ok_or("HOME and XDG_STATE_HOME are unset")?;
    let usage_dir = state_home.join("omarchy/agents/usage");
    fs::create_dir_all(&usage_dir).map_err(|error| error.to_string())?;
    let temporary = usage_dir.join(format!(".codex.omarchy-rs.{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let result = (|| {
        file.write_all(record)?;
        if !record.ends_with(b"\n") {
            file.write_all(b"\n")?;
        }
        file.sync_all()?;
        fs::rename(&temporary, usage_dir.join("codex.json"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| error.to_string())
}

fn current_sibling(name: &str) -> Result<PathBuf, String> {
    let current = env::current_exe().map_err(|error| error.to_string())?;
    Ok(current.with_file_name(name))
}

fn require_absolute_file(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || !path.is_file() {
        return Err(format!(
            "required executable is unavailable: {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_selection_matches_upstream_flags() {
        assert!(codex_is_wanted(&[]).unwrap());
        assert!(codex_is_wanted(&["codex".into()]).unwrap());
        assert!(!codex_is_wanted(&["claude".into()]).unwrap());
        assert!(!codex_is_wanted(&["--except".into(), "codex".into()]).unwrap());
        assert!(codex_is_wanted(&["--force".into()]).unwrap());
    }
}
