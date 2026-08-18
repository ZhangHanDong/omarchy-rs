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
    let activation = omarchy_compat::activation::load_activation();
    let codex_wanted = agent_is_wanted(&args, "codex")?
        && activation
            .as_ref()
            .is_some_and(|config| config.enables("codex"));
    let claude_wanted = agent_is_wanted(&args, "claude")?
        && activation
            .as_ref()
            .is_some_and(|config| config.enables("claude"));
    let octoscode_wanted = agent_is_wanted(&args, "octoscode")?
        && activation
            .as_ref()
            .is_some_and(|config| config.enables("octoscode"));
    if !codex_wanted && !claude_wanted && !octoscode_wanted {
        return command_status(&upstream, &args);
    }

    let mut upstream_args = args.clone();
    if codex_wanted {
        upstream_args.extend(["--except".into(), "codex".into()]);
    }
    if claude_wanted {
        upstream_args.extend(["--except".into(), "claude".into()]);
    }
    if octoscode_wanted {
        upstream_args.extend(["--except".into(), "octoscode".into()]);
    }
    let other_status = command_status(&upstream, &upstream_args)?;
    let mut collector_ok = true;
    if codex_wanted {
        collector_ok &= run_shadow(
            &args,
            "codex",
            "OMARCHY_RS_CODEX_SHADOW",
            "OMARCHY_RS_CODEX_MODE",
        )?;
    }
    if claude_wanted {
        collector_ok &= run_shadow(
            &args,
            "claude",
            "OMARCHY_RS_CLAUDE_SHADOW",
            "OMARCHY_RS_CLAUDE_MODE",
        )?;
    }
    if octoscode_wanted {
        collector_ok &= run_shadow(
            &args,
            "octoscode",
            "OMARCHY_RS_OCTOSCODE_SHADOW",
            "OMARCHY_RS_OCTOSCODE_MODE",
        )?;
    }

    Ok(if other_status == ExitCode::SUCCESS && collector_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn agent_is_wanted(args: &[String], agent: &str) -> Result<bool, String> {
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
    Ok(!excluded.contains(&agent) && (only.is_empty() || only.contains(&agent)))
}

fn run_shadow(
    args: &[String],
    agent: &str,
    env_name: &str,
    mode_env: &str,
) -> Result<bool, String> {
    let default_name = format!("omarchy-agent-usage-{agent}-shadow");
    let shadow = env::var_os(env_name)
        .map(PathBuf::from)
        .unwrap_or(current_sibling(&default_name)?);
    require_absolute_file(&shadow)?;
    let output = Command::new(shadow)
        .env(mode_env, "canary")
        .args(
            args.iter()
                .filter(|arg| matches!(arg.as_str(), "--force" | "--limits-only")),
        )
        .output()
        .map_err(|error| error.to_string())?;
    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
    if !output.status.success() {
        return Ok(false);
    }
    omarchy_compat::shadow::validate_provider_record(&output.stdout, agent)?;
    write_state(agent, &output.stdout)?;
    Ok(true)
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

fn write_state(agent: &str, record: &[u8]) -> Result<(), String> {
    let state_home = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .ok_or("HOME and XDG_STATE_HOME are unset")?;
    let usage_dir = state_home.join("omarchy/agents/usage");
    fs::create_dir_all(&usage_dir).map_err(|error| error.to_string())?;
    let temporary = usage_dir.join(format!(".{agent}.omarchy-rs.{}.tmp", std::process::id()));
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
        fs::rename(&temporary, usage_dir.join(format!("{agent}.json")))
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
    fn provider_selection_matches_upstream_flags() {
        assert!(agent_is_wanted(&[], "codex").unwrap());
        assert!(agent_is_wanted(&["codex".into()], "codex").unwrap());
        assert!(!agent_is_wanted(&["claude".into()], "codex").unwrap());
        assert!(!agent_is_wanted(&["--except".into(), "codex".into()], "codex").unwrap());
        assert!(agent_is_wanted(&["--force".into()], "claude").unwrap());
    }
}
