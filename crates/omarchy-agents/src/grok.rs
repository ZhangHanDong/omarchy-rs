use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Duration, Local, Utc};
use serde_json::{Map, Value, json};

#[derive(Clone, Default)]
struct Tokens {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_creation: u64,
    reasoning: u64,
}

#[derive(Clone)]
struct Turn {
    day: String,
    session: String,
    models: BTreeMap<String, Tokens>,
}

pub fn collect_record(grok_home: &Path) -> Value {
    collect_record_at(grok_home, Local::now())
}

fn collect_record_at(grok_home: &Path, now: DateTime<Local>) -> Value {
    let mut turns = BTreeMap::<String, Turn>::new();
    for path in update_files(grok_home) {
        scan_file(&path, &mut turns);
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
    let mut model_usage = BTreeMap::<String, Tokens>::new();
    let mut today_by_model = BTreeMap::<String, u64>::new();
    let mut today_prompts = 0_u64;
    let mut today_sessions = BTreeSet::new();
    let mut all_sessions = BTreeSet::new();
    let mut active_dates = BTreeSet::new();

    for turn in turns.values() {
        active_dates.insert(turn.day.clone());
        all_sessions.insert(turn.session.clone());
        let total = turn.models.values().fold(0_u64, |sum, tokens| {
            sum.saturating_add(tokens.input.saturating_add(tokens.output))
        });
        if let Some(value) = recent.get_mut(&turn.day) {
            *value = value.saturating_add(total);
        }
        for (model, tokens) in &turn.models {
            add_tokens(model_usage.entry(model.clone()).or_default(), tokens);
            if turn.day == today {
                let value = today_by_model.entry(model.clone()).or_default();
                *value = value.saturating_add(tokens.input.saturating_add(tokens.output));
            }
        }
        if turn.day == today {
            today_prompts = today_prompts.saturating_add(1);
            today_sessions.insert(turn.session.clone());
        }
    }

    let usage = model_usage
        .into_iter()
        .map(|(model, tokens)| {
            (
                model,
                json!({
                    "inputTokens": tokens.input,
                    "outputTokens": tokens.output,
                    "cacheReadInputTokens": tokens.cache_read,
                    "cacheCreationInputTokens": tokens.cache_creation,
                    "reasoningTokens": tokens.reasoning,
                }),
            )
        })
        .collect::<Map<_, _>>();

    json!({
        "schemaVersion": 1,
        "id": "grok",
        "name": "Grok",
        "collectorBackend": "rust",
        "updatedAt": Utc::now().to_rfc3339(),
        "ready": true,
        "hasLocalStats": !turns.is_empty(),
        "tierLabel": "Local",
        "usageStatusText": "",
        "authHelpText": "Run Grok CLI to record usage.",
        "limits": [],
        "todayPrompts": today_prompts,
        "todaySessions": today_sessions.len(),
        "todayTotalTokens": today_by_model.values().sum::<u64>(),
        "todayTokensByModel": today_by_model,
        "recentDays": recent.into_iter().map(|(date, count)| json!({"date": date, "messageCount": count})).collect::<Vec<_>>(),
        "totalPrompts": turns.len(),
        "totalSessions": all_sessions.len(),
        "activeDays": active_dates.len(),
        "activeDates": active_dates,
        "modelUsage": usage,
    })
}

fn scan_file(path: &Path, turns: &mut BTreeMap<String, Turn>) {
    let Ok(file) = File::open(path) else {
        return;
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
        if !bytes
            .windows(b"\"turn_completed\"".len())
            .any(|window| window == b"\"turn_completed\"")
        {
            continue;
        }
        let Ok(root) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        if root["method"] != "_x.ai/session/update"
            || root["params"]["update"]["sessionUpdate"] != "turn_completed"
        {
            continue;
        }
        let Some(event_id) = root["params"]["_meta"]["eventId"]
            .as_str()
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let usage = &root["params"]["update"]["usage"];
        let top = tokens(usage);
        if top.input.saturating_add(top.output) == 0 {
            continue;
        }
        let mut models = BTreeMap::new();
        if let Some(values) = usage["modelUsage"].as_object() {
            for (model, value) in values {
                let count = tokens(value);
                if count.input.saturating_add(count.output) > 0 {
                    models.insert(model.clone(), count);
                }
            }
        }
        if models.is_empty() {
            models.insert("grok".into(), top);
        }
        let Ok(timestamp) = i64::try_from(number(&root["timestamp"])) else {
            continue;
        };
        let Some(at) = DateTime::from_timestamp(timestamp, 0) else {
            continue;
        };
        turns.insert(
            event_id.into(),
            Turn {
                day: at.with_timezone(&Local).date_naive().to_string(),
                session: root["params"]["sessionId"]
                    .as_str()
                    .unwrap_or("grok")
                    .into(),
                models,
            },
        );
    }
}

fn update_files(home: &Path) -> Vec<PathBuf> {
    fn visit(path: &Path, output: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                visit(&path, output);
            } else if file_type.is_file()
                && path.file_name().and_then(|name| name.to_str()) == Some("updates.jsonl")
            {
                output.push(path);
            }
        }
    }
    let mut files = Vec::new();
    visit(&home.join("sessions"), &mut files);
    files.sort();
    files
}

fn tokens(value: &Value) -> Tokens {
    Tokens {
        input: number(&value["inputTokens"]),
        output: number(&value["outputTokens"]),
        cache_read: number(&value["cachedReadTokens"]),
        cache_creation: number(&value["cacheCreationTokens"]),
        reasoning: number(&value["reasoningTokens"]),
    }
}

fn number(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_i64().map(|number| number.max(0) as u64))
        .or_else(|| value.as_str().and_then(|raw| raw.parse::<u64>().ok()))
        .unwrap_or(0)
}

fn add_tokens(target: &mut Tokens, value: &Tokens) {
    target.input = target.input.saturating_add(value.input);
    target.output = target.output.saturating_add(value.output);
    target.cache_read = target.cache_read.saturating_add(value.cache_read);
    target.cache_creation = target.cache_creation.saturating_add(value.cache_creation);
    target.reasoning = target.reasoning.saturating_add(value.reasoning);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/agent_usage/grok")
            .join(path)
    }

    fn copy_fixture(paths: &[&str]) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        for (index, source) in paths.iter().enumerate() {
            let target = home.path().join(format!("sessions/work/session-{index}"));
            fs::create_dir_all(&target).unwrap();
            fs::copy(fixture(source), target.join("updates.jsonl")).unwrap();
        }
        home
    }

    #[test]
    fn grok_fixture_aggregates_completed_turns() {
        let home = copy_fixture(&["valid/session-a.jsonl", "valid/session-b.jsonl"]);
        let now = DateTime::from_timestamp(1_787_155_200, 0)
            .unwrap()
            .with_timezone(&Local);
        let record = collect_record_at(home.path(), now);
        assert_eq!(record["totalPrompts"], 2);
        assert_eq!(record["totalSessions"], 2);
        assert_eq!(record["activeDays"], 2);
        assert_eq!(record["modelUsage"]["grok-4.6"]["inputTokens"], 100);
        assert_eq!(record["modelUsage"]["grok-4.6"]["cacheReadInputTokens"], 40);
        assert_eq!(record["modelUsage"]["grok-4.6"]["reasoningTokens"], 7);
        assert_eq!(record["modelUsage"]["grok-4.6-mini"]["outputTokens"], 10);
    }

    #[test]
    fn grok_malformed_and_duplicate_events() {
        let home = copy_fixture(&["malformed/updates.jsonl"]);
        let record = collect_record(home.path());
        assert_eq!(record["totalPrompts"], 1);
        assert_eq!(record["modelUsage"]["grok"]["inputTokens"], 30);
        assert_eq!(record["modelUsage"]["grok"]["outputTokens"], 4);
    }

    #[test]
    fn grok_empty_home_has_no_local_stats() {
        let home = tempfile::tempdir().unwrap();
        let record = collect_record(home.path());
        assert_eq!(record["totalPrompts"], 0);
        assert_eq!(record["totalSessions"], 0);
        assert_eq!(record["limits"], json!([]));
        assert_eq!(record["hasLocalStats"], false);
    }
}
