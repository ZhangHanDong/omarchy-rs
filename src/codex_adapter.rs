//! Minimal native Codex session adapter.
//!
//! Its accepted record shapes track the ccusage fork revision
//! `302fa5eaf61f7d09a8a2710be0c8fafbc2723e4c`, while the implementation stays
//! inside the single published package. This module intentionally implements
//! only the local event surface used by the Omarchy collector.

use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{CodexServiceTier, CodexUsageEvent};

pub(crate) fn load_codex_events_from_directory(
    sessions_dir: &Path,
) -> Result<Vec<CodexUsageEvent>, String> {
    let mut events = Vec::new();
    for path in usage_files(sessions_dir) {
        scan_file(&path, &mut events);
    }
    Ok(events)
}

fn scan_file(path: &Path, events: &mut Vec<CodexUsageEvent>) {
    let Ok(file) = File::open(path) else { return };
    let fallback_timestamp = file
        .metadata()
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| DateTime::<Utc>::from_timestamp(duration.as_secs() as i64, 0))
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(|| DateTime::<Utc>::UNIX_EPOCH.to_rfc3339());
    let mut model = None;
    let mut service_tier = None;
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    loop {
        line.clear();
        let Ok(read) = reader.read_until(b'\n', &mut line) else {
            break;
        };
        if read == 0 {
            break;
        }
        if !line.windows(6).any(|window| window == b"\"type\"") {
            continue;
        }
        let Ok(entry) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        if entry["type"] == "turn_context" {
            let payload = &entry["payload"];
            model = text(payload, &["model", "model_slug"])
                .map(str::to_owned)
                .or(model);
            service_tier = text(payload, &["service_tier"])
                .and_then(parse_service_tier)
                .or(service_tier);
            continue;
        }
        let mut payload = entry.get("payload").unwrap_or(&entry);
        if entry["type"] == "response_item"
            && let Some(nested) = payload.get("payload")
        {
            payload = nested;
        }
        if payload["type"] != "token_count" {
            continue;
        }
        let usage = &payload["info"]["last_token_usage"];
        let input = number(&usage["input_tokens"]);
        let cached = number(&usage["cached_input_tokens"]);
        let cache_write = number(&usage["cache_write_input_tokens"]);
        let output = number(&usage["output_tokens"]);
        if input.saturating_add(output).saturating_add(cache_write) == 0 {
            continue;
        }
        let timestamp = entry["timestamp"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| fallback_timestamp.clone());
        events.push(CodexUsageEvent {
            session_id: path.to_string_lossy().into_owned(),
            timestamp,
            model: model.clone(),
            input_tokens: input,
            cached_input_tokens: cached.saturating_add(cache_write),
            output_tokens: output,
            reasoning_output_tokens: number(&usage["reasoning_output_tokens"]),
            total_tokens: number(&usage["total_tokens"]),
            service_tier,
        });
    }
}

fn usage_files(root: &Path) -> Vec<PathBuf> {
    fn visit(directory: &Path, output: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                visit(&path, output);
            } else if file_type.is_file() && path.extension().is_some_and(|value| value == "jsonl")
            {
                output.push(path);
            }
        }
    }
    let mut files = Vec::new();
    visit(root, &mut files);
    files.sort();
    files
}

fn text<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| value[*key].as_str())
}

fn parse_service_tier(value: &str) -> Option<CodexServiceTier> {
    match value {
        "fast" => Some(CodexServiceTier::Fast),
        "standard" | "default" => Some(CodexServiceTier::Standard),
        _ => None,
    }
}

fn number(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_i64().map(|number| number.max(0) as u64))
        .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_ccusage_adapter_parses_synthetic_codex_fixture() {
        let sessions =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/agent_usage/codex/valid");
        let events = load_codex_events_from_directory(&sessions).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].input_tokens, 120);
        assert_eq!(events[0].cached_input_tokens, 20);
        assert_eq!(events[0].output_tokens, 30);
        assert_eq!(events[0].total_tokens, 150);
        assert_eq!(events[0].model, None);
    }
}
