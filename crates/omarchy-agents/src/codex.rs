use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::{DateTime, Duration, Local, Utc};
use serde_json::{Value, json};

use crate::load_codex_events_from_directory;

pub fn collect_local_record(sessions_dir: &Path) -> Result<Value, String> {
    let events = load_codex_events_from_directory(sessions_dir)?;
    let today = Local::now().date_naive();
    let today_string = today.to_string();
    let recent_dates: Vec<_> = (0..7)
        .rev()
        .map(|offset| today - Duration::days(offset))
        .collect();
    let mut recent: BTreeMap<String, u64> = recent_dates
        .iter()
        .map(|date| (date.to_string(), 0))
        .collect();
    let mut models: BTreeMap<String, [u64; 4]> = BTreeMap::new();
    let mut sessions = BTreeSet::new();
    let mut today_sessions = BTreeSet::new();
    let mut active_dates = BTreeSet::new();
    let mut today_tokens_by_model: BTreeMap<String, u64> = BTreeMap::new();
    let mut today_prompts = 0_u64;
    let mut today_total_tokens = 0_u64;

    for event in &events {
        let day = local_day(&event.timestamp).unwrap_or_else(|| today_string.clone());
        let model = event.model.as_deref().unwrap_or("codex").to_string();
        let input = event.input_tokens.saturating_sub(event.cached_input_tokens);
        let total = input + event.cached_input_tokens + event.output_tokens;
        sessions.insert(event.session_id.clone());
        active_dates.insert(day.clone());
        let bucket = models.entry(model.clone()).or_default();
        bucket[0] += input;
        bucket[1] += event.output_tokens;
        bucket[2] += event.cached_input_tokens;
        if let Some(value) = recent.get_mut(&day) {
            *value += total;
        }
        if day == today_string {
            today_prompts += 1;
            today_total_tokens += total;
            today_sessions.insert(event.session_id.clone());
            *today_tokens_by_model.entry(model).or_default() += total;
        }
    }

    let model_usage: BTreeMap<_, _> = models
        .into_iter()
        .map(|(model, value)| {
            (
                model,
                json!({
                    "inputTokens": value[0], "outputTokens": value[1],
                    "cacheReadInputTokens": value[2], "cacheCreationInputTokens": value[3]
                }),
            )
        })
        .collect();
    let recent_days: Vec<_> = recent
        .into_iter()
        .map(|(date, message_count)| json!({"date": date, "messageCount": message_count}))
        .collect();

    Ok(json!({
        "schemaVersion": 1, "id": "codex", "name": "Codex",
        "updatedAt": Utc::now().to_rfc3339(), "ready": true, "hasLocalStats": true,
        "todayPrompts": today_prompts, "todaySessions": today_sessions.len(),
        "todayTotalTokens": today_total_tokens, "todayTokensByModel": today_tokens_by_model,
        "recentDays": recent_days, "totalPrompts": events.len(), "totalSessions": sessions.len(),
        "activeDays": active_dates.len(), "activeDates": active_dates, "modelUsage": model_usage,
        "limits": [], "tierLabel": "", "usageStatusText": "Codex unavailable",
        "authHelpText": "codex not found in PATH"
    }))
}

fn local_day(timestamp: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Local).date_naive().to_string())
}
