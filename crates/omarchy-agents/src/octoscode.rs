use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::SystemTime,
};

use chrono::{DateTime, Duration, Local, Utc};
use serde_json::{Map, Value, json};

#[derive(Clone)]
struct Turn {
    day: String,
    session: String,
    model: String,
    input: u64,
    output: u64,
}

pub fn collect_record(octos_home: &Path) -> Value {
    collect_record_at(octos_home, Local::now())
}

fn collect_record_at(octos_home: &Path, now: DateTime<Local>) -> Value {
    let mut models = BTreeMap::<String, String>::new();
    let mut turns = BTreeMap::<String, Turn>::new();
    for path in ledger_files(octos_home) {
        let Ok(file) = File::open(&path) else {
            continue;
        };
        let mut reader = BufReader::new(file);
        let mut bytes = Vec::new();
        loop {
            bytes.clear();
            let Ok(read) = reader.read_until(b'\n', &mut bytes) else {
                break;
            };
            if read == 0 {
                break;
            }
            let line = String::from_utf8_lossy(&bytes);
            if !line.contains("\"turn_id\"") {
                continue;
            }
            let Ok(root) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let event = &root["event"];
            let Some(turn_id) = event["turn_id"].as_str().filter(|value| !value.is_empty()) else {
                continue;
            };
            let metadata = &event["metadata"];
            if metadata["kind"] == "token_cost_update"
                && let Some(model) = metadata["token_cost"]["model"].as_str()
            {
                models.insert(turn_id.into(), model.into());
                continue;
            }
            if event["kind"] != "turn_completed" {
                continue;
            }
            let input = number(&event["tokens_in"]);
            let output = number(&event["tokens_out"]);
            if input + output == 0 {
                continue;
            }
            turns.insert(
                turn_id.into(),
                Turn {
                    day: local_day(&event["session_result"]["message_id"], &path),
                    session: event["session_id"].as_str().unwrap_or("octoscode").into(),
                    model: models
                        .get(turn_id)
                        .cloned()
                        .unwrap_or_else(|| "octoscode".into()),
                    input,
                    output,
                },
            );
        }
    }

    let today = now.date_naive().to_string();
    let mut recent = (0..7)
        .rev()
        .map(|offset| {
            (
                (now.date_naive() - Duration::days(offset)).to_string(),
                0_u64,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut model_usage = BTreeMap::<String, [u64; 2]>::new();
    let mut today_by_model = BTreeMap::<String, u64>::new();
    let mut today_prompts = 0_u64;
    let mut today_sessions = BTreeSet::new();
    let mut all_sessions = BTreeSet::new();
    let mut active_dates = BTreeSet::new();
    for turn in turns.values() {
        let total = turn.input + turn.output;
        active_dates.insert(turn.day.clone());
        all_sessions.insert(turn.session.clone());
        if let Some(value) = recent.get_mut(&turn.day) {
            *value += total;
        }
        let bucket = model_usage.entry(turn.model.clone()).or_default();
        bucket[0] += turn.input;
        bucket[1] += turn.output;
        if turn.day == today {
            today_prompts += 1;
            today_sessions.insert(turn.session.clone());
            *today_by_model.entry(turn.model.clone()).or_default() += total;
        }
    }
    let usage = model_usage
        .into_iter()
        .map(|(model, count)| {
            (
                model,
                json!({
                    "inputTokens": count[0], "outputTokens": count[1],
                    "cacheReadInputTokens": 0, "cacheCreationInputTokens": 0
                }),
            )
        })
        .collect::<Map<_, _>>();
    json!({
        "schemaVersion": 1, "id": "octoscode", "name": "Octoscode",
        "collectorBackend": "rust", "updatedAt": Utc::now().to_rfc3339(),
        "ready": true, "hasLocalStats": !turns.is_empty(), "tierLabel": "Local",
        "usageStatusText": "", "authHelpText": "Run Octoscode to record usage.", "limits": [],
        "todayPrompts": today_prompts, "todaySessions": today_sessions.len(),
        "todayTotalTokens": today_by_model.values().sum::<u64>(), "todayTokensByModel": today_by_model,
        "recentDays": recent.into_iter().map(|(date, count)| json!({"date":date,"messageCount":count})).collect::<Vec<_>>(),
        "totalPrompts": turns.len(), "totalSessions": all_sessions.len(),
        "activeDays": active_dates.len(), "activeDates": active_dates, "modelUsage": usage,
    })
}

fn ledger_files(home: &Path) -> Vec<PathBuf> {
    fn visit(path: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, out);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("ledger-") && name.ends_with(".log"))
            {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    visit(&home.join("instances"), &mut files);
    files.sort();
    files
}

fn number(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_i64().map(|n| n.max(0) as u64))
        .or_else(|| {
            value
                .as_str()
                .and_then(|raw| raw.parse::<i64>().ok())
                .map(|n| n.max(0) as u64)
        })
        .unwrap_or(0)
}

fn local_day(message_id: &Value, fallback: &Path) -> String {
    if let Some(raw) = message_id
        .as_str()
        .and_then(|value| value.rsplit(':').next())
        && let Ok(nanos) = raw.parse::<i64>()
        && let Some(at) =
            DateTime::from_timestamp(nanos / 1_000_000_000, (nanos % 1_000_000_000) as u32)
    {
        return at.with_timezone(&Local).date_naive().to_string();
    }
    fs::metadata(fallback)
        .and_then(|meta| meta.modified())
        .ok()
        .map(|at: SystemTime| DateTime::<Local>::from(at).date_naive().to_string())
        .unwrap_or_else(|| Local::now().date_naive().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/agent_usage/octoscode")
            .join(name)
    }
    fn home_with(name: &str) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let target = home.path().join("instances/i/ui-protocol/s");
        fs::create_dir_all(&target).unwrap();
        fs::copy(fixture(name), target.join("ledger-synthetic.log")).unwrap();
        home
    }

    #[test]
    fn octoscode_fixture_parity() {
        let home = home_with("valid/ledger-synthetic.log");
        let now = DateTime::parse_from_rfc3339("2026-01-02T12:00:00+00:00")
            .unwrap()
            .with_timezone(&Local);
        let record = collect_record_at(home.path(), now);
        assert_eq!(record["totalPrompts"], 2);
        assert_eq!(record["totalSessions"], 1);
        assert_eq!(record["modelUsage"]["k3"]["inputTokens"], 100);
        assert_eq!(record["modelUsage"]["octoscode"]["outputTokens"], 5);
    }

    #[test]
    fn octoscode_malformed_and_repeated_turns() {
        let home = home_with("malformed/ledger-synthetic.log");
        let record = collect_record(home.path());
        assert_eq!(record["totalPrompts"], 1);
        assert_eq!(record["modelUsage"]["octoscode"]["inputTokens"], 30);
        assert_eq!(record["modelUsage"]["octoscode"]["outputTokens"], 4);
    }

    #[test]
    fn octoscode_streams_large_ledgers() {
        let home = tempfile::tempdir().unwrap();
        let target = home.path().join("instances/i/ui-protocol/s");
        fs::create_dir_all(&target).unwrap();
        let mut ledger = File::create(target.join("ledger-large.log")).unwrap();
        for _ in 0..200_000 {
            ledger.write_all(b"ignored-\xff-line\n").unwrap();
        }
        ledger
            .write_all(&fs::read(fixture("valid/ledger-synthetic.log")).unwrap())
            .unwrap();
        drop(ledger);

        let record = collect_record(home.path());
        assert_eq!(record["totalPrompts"], 2);
        assert_eq!(record["modelUsage"]["k3"]["inputTokens"], 100);
    }
}
