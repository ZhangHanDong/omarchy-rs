use std::{
    env,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

const AUTH_HELP: &str = "Run `codex login` to authenticate.";

pub fn fetch_codex_rpc() -> Value {
    let Some(codex) = find_command("codex") else {
        return unavailable("codex not found in PATH");
    };
    match fetch_from_executable(&codex) {
        Ok(value) => value,
        Err(error) => json!({
            "limits": [], "tierLabel": "", "usageStatusText": "Codex limits unavailable",
            "authHelpText": error,
        }),
    }
}

fn fetch_from_executable(codex: &Path) -> Result<Value, String> {
    let mut child = Command::new(codex)
        .args(["-s", "read-only", "-a", "untrusted", "app-server"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut stdin = child.stdin.take().ok_or("Codex stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("Codex stdout unavailable")?;
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(message) = serde_json::from_str::<Value>(&line)
                && sender.send(message).is_err()
            {
                break;
            }
        }
    });

    let result = (|| {
        rpc_request(
            &mut stdin,
            &receiver,
            1,
            "initialize",
            json!({"clientInfo": {"name": "omarchy-agent-usage", "version": "1"}}),
            Duration::from_secs(8),
        )?;
        write_message(&mut stdin, &json!({"method": "initialized", "params": {}}))?;
        let account = rpc_request(
            &mut stdin,
            &receiver,
            2,
            "account/read",
            json!({}),
            Duration::from_secs(4),
        )?;
        let limits = rpc_request(
            &mut stdin,
            &receiver,
            3,
            "account/rateLimits/read",
            json!({}),
            Duration::from_secs(4),
        )?;
        Ok(parse_rpc_records(&account, &limits))
    })();

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    result
}

fn rpc_request(
    stdin: &mut ChildStdin,
    receiver: &Receiver<Value>,
    id: u64,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    write_message(
        stdin,
        &json!({"id": id, "method": method, "params": params}),
    )?;
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| method.to_string())?;
        let message = receiver
            .recv_timeout(remaining)
            .map_err(|_| method.to_string())?;
        if message.get("id").and_then(Value::as_u64) == Some(id) {
            return Ok(message);
        }
    }
}

fn write_message(stdin: &mut ChildStdin, message: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *stdin, message).map_err(|error| error.to_string())?;
    stdin.write_all(b"\n").map_err(|error| error.to_string())?;
    stdin.flush().map_err(|error| error.to_string())
}

fn parse_rpc_records(account_message: &Value, limits_message: &Value) -> Value {
    let account = account_message
        .pointer("/result/account")
        .and_then(Value::as_object);
    let limits = limits_message
        .pointer("/result/rateLimits")
        .and_then(Value::as_object);
    let plan = limits
        .and_then(|value| value.get("planType"))
        .or_else(|| account.and_then(|value| value.get("planType")))
        .or_else(|| account.and_then(|value| value.get("type")))
        .and_then(Value::as_str)
        .unwrap_or("");
    let windows = ["primary", "secondary"]
        .into_iter()
        .filter_map(|name| limits.and_then(|value| value.get(name)))
        .filter_map(limit_window)
        .collect::<Vec<_>>();
    json!({
        "limits": windows, "tierLabel": plan, "usageStatusText": "",
        "authHelpText": AUTH_HELP,
    })
}

fn limit_window(window: &Value) -> Option<Value> {
    let used = window.get("usedPercent")?.as_f64()?;
    let minutes = number(window.get("windowDurationMins"));
    let label = if minutes == 10_080 {
        "Weekly (7-day)".to_string()
    } else if minutes != 0 && minutes % 60 == 0 {
        format!("{}h window", minutes / 60)
    } else if minutes != 0 {
        format!("{minutes}m window")
    } else {
        "Limit".to_string()
    };
    let reset = number(window.get("resetsAt"));
    let resets_at = (reset != 0)
        .then(|| DateTime::<Utc>::from_timestamp(reset, 0))
        .flatten()
        .map(|value| value.to_rfc3339())
        .unwrap_or_default();
    Some(json!({"label": label, "percent": used / 100.0, "resetsAt": resets_at}))
}

fn number(value: Option<&Value>) -> i64 {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        })
        .unwrap_or(0)
}

fn unavailable(help: &str) -> Value {
    json!({
        "limits": [], "tierLabel": "", "usageStatusText": "Codex unavailable",
        "authHelpText": help,
    })
}

fn find_command(name: &str) -> Option<PathBuf> {
    let mut directories =
        env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        directories.extend([
            home.join(".local/bin"),
            home.join(".npm-global/bin"),
            home.join(".local/share/mise/shims"),
        ]);
    }
    directories
        .into_iter()
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::{fs, os::unix::fs::PermissionsExt};

    #[test]
    fn codex_rpc_fields_match_upstream_shape() {
        let account = json!({"result": {"account": {"planType": "fallback"}}});
        let limits = json!({"result": {"rateLimits": {
            "planType": "pro",
            "primary": {"usedPercent": 25, "windowDurationMins": 300, "resetsAt": 0},
            "secondary": {"usedPercent": 50.0, "windowDurationMins": "10080"}
        }}});
        assert_eq!(
            parse_rpc_records(&account, &limits),
            json!({
                "limits": [
                    {"label": "5h window", "percent": 0.25, "resetsAt": ""},
                    {"label": "Weekly (7-day)", "percent": 0.5, "resetsAt": ""}
                ],
                "tierLabel": "pro", "usageStatusText": "", "authHelpText": AUTH_HELP
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn codex_rpc_fake_app_server_round_trip() {
        let isolated = tempfile::TempDir::new().unwrap();
        let fake = isolated.path().join("codex");
        fs::write(
            &fake,
            r#"#!/bin/sh
read -r initialize
printf '%s\n' '{"id":1,"result":{}}'
read -r initialized
read -r account
printf '%s\n' '{"id":2,"result":{"account":{"planType":"team"}}}'
read -r limits
printf '%s\n' '{"id":3,"result":{"rateLimits":{"primary":{"usedPercent":10,"windowDurationMins":60}}}}'
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&fake, permissions).unwrap();

        let record = fetch_from_executable(&fake).unwrap();
        assert_eq!(record["tierLabel"], "team");
        assert_eq!(record["limits"][0]["label"], "1h window");
        assert_eq!(record["limits"][0]["percent"], 0.1);
    }
}
