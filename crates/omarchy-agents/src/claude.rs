use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    time::{Duration as StdDuration, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Duration, Local, Utc};
use serde_json::{Map, Value, json};

const AUTH_HELP: &str = "Run `claude auth login` to restore authoritative usage.";
const DEFAULT_USAGE_ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";
const PROBE_MIN_INTERVAL_SECONDS: u64 = 15;

#[derive(Clone, Debug)]
pub struct CollectOptions {
    pub force: bool,
    pub endpoint: String,
    pub cache_root: PathBuf,
}

impl CollectOptions {
    pub fn from_environment(force: bool) -> Result<Self, String> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is unset")?;
        Ok(Self {
            force,
            endpoint: env::var("OMARCHY_RS_CLAUDE_USAGE_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_USAGE_ENDPOINT.into()),
            cache_root: env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".cache"))
                .join("omarchy/agent-usage"),
        })
    }
}

pub fn collect_record(config_dir: &Path, options: &CollectOptions) -> Result<Value, String> {
    let mut stats = collect_local_record(config_dir)?;
    if stats["totalPrompts"].as_u64().unwrap_or(0) == 0
        && let Some(fallback) = stats_cache_fallback(config_dir)
    {
        stats = fallback;
    }
    let (access_token, expires_at_ms, tier_label) = oauth_login(config_dir);
    let limits = collect_limits(&access_token, expires_at_ms, options)?;
    let mut record = stats
        .as_object()
        .cloned()
        .ok_or("stats are not an object")?;
    record.extend([
        ("schemaVersion".into(), 1.into()),
        ("id".into(), "claude".into()),
        ("name".into(), "Claude Code".into()),
        ("collectorBackend".into(), "rust".into()),
        ("updatedAt".into(), Utc::now().to_rfc3339().into()),
        (
            "ready".into(),
            (stats["totalPrompts"].as_u64().unwrap_or(0) > 0 || !limits.limits.is_empty()).into(),
        ),
        ("hasLocalStats".into(), true.into()),
        ("tierLabel".into(), tier_label.into()),
        ("limits".into(), Value::Array(limits.limits)),
        ("usageStatusText".into(), limits.status.into()),
        ("authHelpText".into(), limits.help.into()),
    ]);
    if limits.retry_advised {
        record.insert("retryAdvised".into(), true.into());
    }
    Ok(Value::Object(record))
}

pub fn collect_local_record(config_dir: &Path) -> Result<Value, String> {
    let today = Local::now().date_naive();
    let today_string = today.to_string();
    let mut recent: BTreeMap<String, u64> = (0..7)
        .rev()
        .map(|offset| ((today - Duration::days(offset)).to_string(), 0))
        .collect();
    let mut usage_by_model: BTreeMap<String, [u64; 4]> = BTreeMap::new();
    let mut sessions = BTreeSet::new();
    let mut active_dates = BTreeSet::new();
    let mut today_sessions = BTreeSet::new();
    let mut today_tokens = BTreeMap::<String, u64>::new();
    let mut today_prompts = 0_u64;
    let mut today_total_tokens = 0_u64;
    let mut prompts = 0_u64;
    let mut seen = BTreeSet::new();

    for path in ordered_usage_files(config_dir) {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        for (line_index, line) in content.lines().enumerate() {
            if !line.contains("\"usage\":") {
                continue;
            }
            let Ok(entry) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let message = entry["message"].as_object();
            if entry["type"] != "assistant"
                && message
                    .and_then(|value| value.get("role"))
                    .and_then(Value::as_str)
                    != Some("assistant")
            {
                continue;
            }
            let usage = message
                .and_then(|value| value.get("usage"))
                .or_else(|| entry.get("usage"));
            let Some(usage) = usage.and_then(Value::as_object) else {
                continue;
            };
            let message_id = message
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                .or_else(|| entry["messageId"].as_str());
            let unique = message_id.map(str::to_string).unwrap_or_else(|| {
                let fallback = entry["uuid"]
                    .as_str()
                    .or_else(|| entry["requestId"].as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| (line_index + 1).to_string());
                format!("{}:{fallback}", path.display())
            });
            if !seen.insert(unique) {
                continue;
            }
            let input = usage_number(usage, "input_tokens", "inputTokens");
            let output = usage_number(usage, "output_tokens", "outputTokens");
            let cache_read = usage_number(usage, "cache_read_input_tokens", "cacheReadInputTokens");
            let cache_creation = usage_number(
                usage,
                "cache_creation_input_tokens",
                "cacheCreationInputTokens",
            );
            let total = input + output + cache_read + cache_creation;
            if total == 0 {
                continue;
            }
            let model = message
                .and_then(|value| value.get("model"))
                .or_else(|| entry.get("model"))
                .and_then(Value::as_str)
                .unwrap_or("claude")
                .to_string();
            let day = local_day(
                entry
                    .get("timestamp")
                    .or_else(|| message.and_then(|value| value.get("timestamp"))),
            );
            let session = entry["sessionId"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| path.display().to_string());
            sessions.insert(session.clone());
            active_dates.insert(day.clone());
            prompts += 1;
            let bucket = usage_by_model.entry(model.clone()).or_default();
            bucket[0] += input;
            bucket[1] += output;
            bucket[2] += cache_read;
            bucket[3] += cache_creation;
            if let Some(count) = recent.get_mut(&day) {
                *count += total;
            }
            if day == today_string {
                today_prompts += 1;
                today_total_tokens += total;
                today_sessions.insert(session);
                *today_tokens.entry(model).or_default() += total;
            }
        }
    }

    let model_usage = usage_by_model
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
        .collect::<Map<_, _>>();
    Ok(json!({
        "todayPrompts": today_prompts,
        "todaySessions": today_sessions.len(),
        "todayTotalTokens": today_total_tokens,
        "todayTokensByModel": today_tokens,
        "recentDays": recent.into_iter().map(|(date, message_count)| json!({"date": date, "messageCount": message_count})).collect::<Vec<_>>(),
        "modelUsage": model_usage,
        "totalPrompts": prompts,
        "totalSessions": sessions.len(),
        "activeDays": active_dates.len(),
        "activeDates": active_dates,
    }))
}

fn ordered_usage_files(config_dir: &Path) -> Vec<PathBuf> {
    fn visit(directory: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                visit(&path, out);
            } else if file_type.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "jsonl")
            {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    visit(&config_dir.join("projects"), &mut files);
    files.sort();
    files
}

fn usage_number(usage: &Map<String, Value>, snake: &str, camel: &str) -> u64 {
    usage
        .get(snake)
        .or_else(|| usage.get(camel))
        .map(number)
        .unwrap_or(0)
}

fn local_day(value: Option<&Value>) -> String {
    let today = Local::now().date_naive().to_string();
    let Some(value) = value else { return today };
    if let Some(raw) = value.as_str()
        && let Ok(parsed) = DateTime::parse_from_rfc3339(raw)
    {
        return parsed.with_timezone(&Local).date_naive().to_string();
    }
    let Some(raw) = value.as_f64() else {
        return today;
    };
    let seconds = if raw > 10_000_000_000.0 {
        raw / 1000.0
    } else {
        raw
    };
    DateTime::from_timestamp(seconds as i64, 0)
        .map(|parsed| parsed.with_timezone(&Local).date_naive().to_string())
        .unwrap_or(today)
}

fn stats_cache_fallback(config_dir: &Path) -> Option<Value> {
    let data: Value =
        serde_json::from_slice(&fs::read(config_dir.join("stats-cache.json")).ok()?).ok()?;
    let today = Local::now().date_naive().to_string();
    let today_tokens = data["dailyModelTokens"]
        .as_array()?
        .iter()
        .find(|entry| entry["date"] == today)
        .and_then(|entry| entry["tokensByModel"].as_object())
        .cloned()
        .unwrap_or_default();
    let activity = data["dailyActivity"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let active_dates = activity
        .iter()
        .filter(|entry| number(&entry["messageCount"]) > 0)
        .filter_map(|entry| entry["date"].as_str().map(str::to_string))
        .collect::<BTreeSet<_>>();
    let (today_prompts, today_sessions) = today_prompts_from_history(config_dir);
    Some(json!({
        "todayPrompts": today_prompts, "todaySessions": today_sessions,
        "todayTotalTokens": today_tokens.values().map(number).sum::<u64>(),
        "todayTokensByModel": today_tokens,
        "recentDays": activity.into_iter().rev().take(7).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
        "modelUsage": data["modelUsage"].clone(),
        "totalPrompts": number(&data["totalMessages"]), "totalSessions": number(&data["totalSessions"]),
        "activeDays": active_dates.len(), "activeDates": active_dates,
    }))
}

fn today_prompts_from_history(config_dir: &Path) -> (u64, usize) {
    let Ok(content) = fs::read_to_string(config_dir.join("history.jsonl")) else {
        return (0, 0);
    };
    let today = Local::now().date_naive();
    let mut prompts = 0;
    let mut sessions = BTreeSet::new();
    for line in content.lines().rev() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(timestamp) = entry["timestamp"].as_f64() else {
            continue;
        };
        let seconds = if timestamp > 10_000_000_000.0 {
            timestamp / 1000.0
        } else {
            timestamp
        };
        let Some(at) = DateTime::from_timestamp(seconds as i64, 0) else {
            continue;
        };
        if at.with_timezone(&Local).date_naive() != today {
            break;
        }
        prompts += 1;
        if let Some(session) = entry["sessionId"].as_str() {
            sessions.insert(session.to_string());
        }
    }
    (prompts, sessions.len())
}

fn number(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_f64().map(|n| n.round().max(0.0) as u64))
        .unwrap_or(0)
}

fn oauth_login(config_dir: &Path) -> (String, u64, String) {
    let Ok(bytes) = fs::read(config_dir.join(".credentials.json")) else {
        return (String::new(), 0, String::new());
    };
    let Ok(data) = serde_json::from_slice::<Value>(&bytes) else {
        return (String::new(), 0, String::new());
    };
    let login = &data["claudeAiOauth"];
    let tier = login["rateLimitTier"].as_str().unwrap_or("");
    let subscription = login["subscriptionType"].as_str().unwrap_or("");
    (
        login["accessToken"].as_str().unwrap_or("").to_string(),
        number(&login["expiresAt"]),
        plan_label(tier, subscription),
    )
}

pub fn plan_label(tier: &str, subscription: &str) -> String {
    let lower = tier.to_ascii_lowercase();
    if let Some(rest) = lower.split("max_").nth(1)
        && let Some(multiplier) = rest.split(['_', '-']).next()
        && multiplier.ends_with('x')
        && multiplier[..multiplier.len() - 1]
            .chars()
            .all(|c| c.is_ascii_digit())
    {
        return format!("Max {multiplier}");
    }
    let mut chars = subscription.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

#[derive(Debug)]
struct LimitsResult {
    limits: Vec<Value>,
    status: String,
    help: String,
    retry_advised: bool,
}

fn collect_limits(
    token: &str,
    expires_at_ms: u64,
    options: &CollectOptions,
) -> Result<LimitsResult, String> {
    fs::create_dir_all(&options.cache_root).map_err(|error| error.to_string())?;
    let cache_path = options.cache_root.join("claude-limits.json");
    let cached: Value = fs::read(&cache_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| json!({}));
    let fallback = usable_cached_limits(&cached);
    let now_ms = now_ms();
    if token.is_empty() {
        return Ok(LimitsResult {
            limits: fallback,
            status: "Waiting for auth".into(),
            help: AUTH_HELP.into(),
            retry_advised: false,
        });
    }
    if expires_at_ms > 0 && expires_at_ms <= now_ms {
        let suffix = if fallback.is_empty() {
            "."
        } else {
            " — showing the last known limits."
        };
        return Ok(LimitsResult {
            limits: fallback,
            status: "Sign-in expired".into(),
            help: format!(
                "Claude Code's saved sign-in expired{suffix} Start Claude Code, or run `claude auth login`, to refresh it."
            ),
            retry_advised: false,
        });
    }
    let fetched = number(&cached["fetchedAtMs"]);
    if !options.force
        && !fallback.is_empty()
        && now_ms.saturating_sub(fetched) < PROBE_MIN_INTERVAL_SECONDS * 1000
    {
        return Ok(LimitsResult {
            limits: fallback,
            status: String::new(),
            help: AUTH_HELP.into(),
            retry_advised: false,
        });
    }
    match probe_limits(token, &options.endpoint) {
        Ok(limits) if !limits.is_empty() => {
            atomic_json(&cache_path, &json!({"fetchedAtMs": now_ms, "limits": limits}))?;
            Ok(LimitsResult { limits, status: String::new(), help: AUTH_HELP.into(), retry_advised: false })
        }
        Ok(_) => Ok(LimitsResult { limits: fallback, status: "Claude limits unavailable".into(), help: "Anthropic's usage endpoint returned no limits. Local Claude Code stats are still shown.".into(), retry_advised: false }),
        Err(error) => Ok(LimitsResult { limits: fallback, status: "Claude limits unavailable".into(), help: error, retry_advised: true }),
    }
}

fn probe_limits(token: &str, endpoint: &str) -> Result<Vec<Value>, String> {
    let response = ureq::get(endpoint)
        .set("Authorization", &format!("Bearer {token}"))
        .set("anthropic-beta", "oauth-2025-04-20")
        .set("Accept", "application/json")
        .timeout(StdDuration::from_secs(10))
        .call()
        .map_err(|error| format!("Couldn't reach Anthropic's usage endpoint ({error}). Local Claude Code stats are still shown."))?;
    let payload: Value = response.into_json().map_err(|error| error.to_string())?;
    Ok(normalize_limits_payload(&payload))
}

pub fn normalize_limits_payload(payload: &Value) -> Vec<Value> {
    let session = payload.get("five_hour").and_then(Value::as_object);
    let weekly = payload
        .get("seven_day_oauth_apps")
        .and_then(Value::as_object)
        .or_else(|| payload.get("seven_day").and_then(Value::as_object));
    let scoped = payload["limits"].as_array().cloned().unwrap_or_default();
    let raw = session
        .into_iter()
        .chain(weekly)
        .filter_map(|bucket| bucket.get("utilization").and_then(Value::as_f64))
        .chain(scoped.iter().filter_map(|entry| entry["percent"].as_f64()))
        .collect::<Vec<_>>();
    let percent_scale = raw.iter().any(|value| *value >= 1.0);
    let mut out = Vec::new();
    if let Some(bucket) = session {
        push_limit(
            &mut out,
            "Session (5-hour)",
            bucket.get("utilization"),
            bucket.get("resets_at"),
            percent_scale,
            None,
        );
    }
    if let Some(bucket) = weekly {
        push_limit(
            &mut out,
            "Weekly (7-day)",
            bucket.get("utilization"),
            bucket.get("resets_at"),
            percent_scale,
            None,
        );
    }
    let mut seen = BTreeSet::new();
    for entry in scoped {
        let Some(model) = entry.pointer("/scope/model").and_then(Value::as_object) else {
            continue;
        };
        let name = model
            .get("display_name")
            .or_else(|| model.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let kind = entry["kind"].as_str().unwrap_or("");
        if name.is_empty() || !seen.insert((name.to_string(), kind.to_string())) {
            continue;
        }
        let window = if kind.to_ascii_lowercase().contains("month") {
            "Monthly"
        } else if kind.to_ascii_lowercase().contains("week")
            || kind.to_ascii_lowercase().contains("day")
        {
            "Weekly"
        } else if kind.to_ascii_lowercase().contains("hour")
            || kind.to_ascii_lowercase().contains("session")
        {
            "Session"
        } else {
            ""
        };
        let title = format!(
            "{name}{}",
            if window.is_empty() {
                String::new()
            } else {
                format!(" {window}")
            }
        );
        push_limit(
            &mut out,
            &title,
            entry.get("percent"),
            entry.get("resets_at"),
            percent_scale,
            Some(&title),
        );
    }
    out
}

fn push_limit(
    out: &mut Vec<Value>,
    label: &str,
    raw: Option<&Value>,
    reset: Option<&Value>,
    percent_scale: bool,
    title: Option<&str>,
) {
    let Some(mut value) = raw.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str()?.trim_end_matches('%').parse().ok())
    }) else {
        return;
    };
    if value < 0.0 {
        return;
    }
    if percent_scale || value > 1.0 {
        value /= 100.0;
    }
    let mut limit =
        json!({"label": label, "percent": value.min(1.0), "resetsAt": normalize_reset(reset)});
    if let Some(title) = title {
        limit["title"] = title.into();
    }
    out.push(limit);
}

fn normalize_reset(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    if let Some(number) = value.as_i64() {
        let seconds = if number < 1_000_000_000_000 {
            number
        } else {
            number / 1000
        };
        return DateTime::from_timestamp(seconds, 0)
            .map(|at| at.to_rfc3339())
            .unwrap_or_else(|| number.to_string());
    }
    let raw = value.as_str().unwrap_or("").trim();
    if raw.is_empty() {
        return String::new();
    }
    DateTime::parse_from_rfc3339(raw)
        .map(|at| at.to_rfc3339())
        .unwrap_or_else(|_| raw.to_string())
}

fn usable_cached_limits(cached: &Value) -> Vec<Value> {
    let now = Utc::now();
    cached["limits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| {
            let raw = entry["resetsAt"].as_str().unwrap_or("");
            raw.is_empty()
                || DateTime::parse_from_rfc3339(raw)
                    .map(|at| at > now)
                    .unwrap_or(true)
        })
        .cloned()
        .collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn atomic_json(path: &Path, value: &Value) -> Result<(), String> {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(
        &temporary,
        serde_json::to_vec(value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/agent_usage/claude")
            .join(name)
    }

    #[test]
    fn claude_fixture_parity() {
        let config = tempfile::tempdir().unwrap();
        let project = config.path().join("projects/synthetic");
        fs::create_dir_all(&project).unwrap();
        fs::copy(
            fixture("valid/transcript.jsonl"),
            project.join("session.jsonl"),
        )
        .unwrap();
        let record = collect_local_record(config.path()).unwrap();
        assert_eq!(record["totalPrompts"], 1);
        assert_eq!(record["totalSessions"], 1);
        assert_eq!(record["modelUsage"]["claude-synthetic"]["inputTokens"], 100);
        assert_eq!(
            record["modelUsage"]["claude-synthetic"]["cacheCreationInputTokens"],
            10
        );
        assert_eq!(
            record["modelUsage"]["claude-synthetic"]["cacheReadInputTokens"],
            20
        );
        assert_eq!(record["modelUsage"]["claude-synthetic"]["outputTokens"], 25);
    }

    #[test]
    fn claude_malformed_and_duplicate_records() {
        let config = tempfile::tempdir().unwrap();
        let project = config.path().join("projects/synthetic");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("session.jsonl"), concat!(
            "not json\n",
            "{\"type\":\"assistant\",\"timestamp\":\"2026-01-02T03:05:00Z\",\"sessionId\":\"s\",\"requestId\":\"r\",\"message\":{\"id\":\"m\",\"model\":\"claude\",\"usage\":{\"input_tokens\":10,\"output_tokens\":2}}}\n",
            "{\"type\":\"assistant\",\"timestamp\":\"2026-01-02T03:05:00Z\",\"sessionId\":\"s\",\"requestId\":\"r\",\"message\":{\"id\":\"m\",\"model\":\"claude\",\"usage\":{\"input_tokens\":10,\"output_tokens\":2}}}\n"
        )).unwrap();
        assert_eq!(
            collect_local_record(config.path()).unwrap()["totalPrompts"],
            1
        );
    }

    #[test]
    fn claude_stats_cache_and_history_fallback() {
        let config = tempfile::tempdir().unwrap();
        fs::create_dir_all(config.path().join("projects")).unwrap();
        fs::write(config.path().join("stats-cache.json"), r#"{"totalMessages":12,"totalSessions":3,"dailyActivity":[{"date":"2026-01-02","messageCount":2}],"dailyModelTokens":[],"modelUsage":{"claude":{"inputTokens":7}}}"#).unwrap();
        let options = CollectOptions {
            force: false,
            endpoint: "http://127.0.0.1:1".into(),
            cache_root: config.path().join("cache"),
        };
        let record = collect_record(config.path(), &options).unwrap();
        assert_eq!(record["totalPrompts"], 12);
        assert_eq!(record["totalSessions"], 3);
        assert_eq!(record["collectorBackend"], "rust");
    }

    #[test]
    fn claude_limits_payload_parity() {
        let payload = json!({
            "five_hour": {"utilization": 37.0, "resets_at": "2030-01-01T00:00:00Z"},
            "seven_day_oauth_apps": {"utilization": 1.0, "resets_at": "2030-01-02T00:00:00Z"},
            "limits": [{"kind":"weekly_scoped","percent":20.0,"resets_at":"2030-01-03T00:00:00Z","scope":{"model":{"display_name":"Opus"}}}]
        });
        let limits = normalize_limits_payload(&payload);
        assert_eq!(limits.len(), 3);
        assert_eq!(limits[0]["percent"], 0.37);
        assert_eq!(limits[1]["percent"], 0.01);
        assert_eq!(limits[2]["title"], "Opus Weekly");
        assert_eq!(plan_label("default_claude_max_20x", ""), "Max 20x");
    }

    #[test]
    fn claude_limits_falls_back_from_null_oauth_weekly() {
        let payload = json!({
            "five_hour": {"utilization": 25.0, "resets_at": "2030-01-01T00:00:00Z"},
            "seven_day_oauth_apps": null,
            "seven_day": {"utilization": 40.0, "resets_at": "2030-01-02T00:00:00Z"},
            "limits": [{
                "kind": "weekly_scoped",
                "percent": 20.0,
                "resets_at": "2030-01-03T00:00:00Z",
                "scope": {"model": {"display_name": "Fable"}}
            }]
        });
        let limits = normalize_limits_payload(&payload);
        assert_eq!(limits.len(), 3);
        assert_eq!(limits[1]["label"], "Weekly (7-day)");
        assert_eq!(limits[1]["percent"], 0.4);
        assert_eq!(limits[2]["title"], "Fable Weekly");
    }

    #[test]
    fn claude_missing_and_expired_credentials() {
        let config = tempfile::tempdir().unwrap();
        fs::create_dir_all(config.path().join("projects")).unwrap();
        let options = CollectOptions {
            force: false,
            endpoint: "http://127.0.0.1:1/must-not-run".into(),
            cache_root: config.path().join("cache"),
        };
        let missing = collect_record(config.path(), &options).unwrap();
        assert_eq!(missing["usageStatusText"], "Waiting for auth");
        fs::write(
            config.path().join(".credentials.json"),
            r#"{"claudeAiOauth":{"accessToken":"synthetic-secret","expiresAt":1}}"#,
        )
        .unwrap();
        let expired = collect_record(config.path(), &options).unwrap();
        assert_eq!(expired["usageStatusText"], "Sign-in expired");
        assert!(!expired.to_string().contains("synthetic-secret"));
    }
}
