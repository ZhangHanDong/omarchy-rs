use std::{collections::BTreeMap, path::Path};

use serde_json::Value;

const COMPARED_FIELDS: &[&str] = &[
    "hasLocalStats",
    "todayPrompts",
    "todaySessions",
    "todayTotalTokens",
    "todayTokensByModel",
    "recentDays",
    "totalPrompts",
    "totalSessions",
    "activeDays",
    "activeDates",
    "modelUsage",
    "limits",
    "tierLabel",
    "usageStatusText",
    "authHelpText",
];

#[derive(Debug, Eq, PartialEq)]
pub struct ShadowComparison {
    pub local_fields_match: bool,
    pub differing_fields: Vec<String>,
}

pub fn canary_eligible(
    upstream_fingerprint_matches: bool,
    has_external_sources: bool,
    limits_only: bool,
    candidate: &Value,
) -> bool {
    upstream_fingerprint_matches
        && !has_external_sources
        && !limits_only
        && validate_record(&serde_json::to_vec(candidate).unwrap_or_default()).is_ok()
        && candidate.get("usageStatusText").and_then(Value::as_str)
            != Some("Codex limits unavailable")
}

pub fn validate_record(bytes: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "collector record is not a JSON object".to_string())?;
    for field in ["schemaVersion", "id", "name", "ready", "limits"] {
        if !object.contains_key(field) {
            return Err(format!("collector record is missing {field}"));
        }
    }
    if value["schemaVersion"] != 1 || value["id"] != "codex" {
        return Err("collector record identity is incompatible".into());
    }
    Ok(value)
}

pub fn compare_local_fields(candidate: &Value, upstream: &Value) -> ShadowComparison {
    let differing_fields = COMPARED_FIELDS
        .iter()
        .filter(|field| candidate.get(**field) != upstream.get(**field))
        .map(|field| (*field).to_string())
        .collect::<Vec<_>>();
    ShadowComparison {
        local_fields_match: differing_fields.is_empty(),
        differing_fields,
    }
}

pub fn sanitized_receipt(comparison: &ShadowComparison) -> Value {
    let counts = comparison.differing_fields.iter().fold(
        BTreeMap::<&str, usize>::new(),
        |mut counts, field| {
            *counts.entry(field).or_default() += 1;
            counts
        },
    );
    serde_json::json!({
        "schemaVersion": 1,
        "localFieldsMatch": comparison.local_fields_match,
        "differingFields": counts.keys().collect::<Vec<_>>(),
    })
}

pub fn verified_absolute_executable(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || !path.is_file() {
        return Err("upstream collector must be an existing absolute file".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(prompts: u64) -> Value {
        serde_json::json!({
            "schemaVersion": 1, "id": "codex", "name": "Codex", "ready": true,
            "limits": [], "hasLocalStats": true, "todayPrompts": prompts,
            "todaySessions": 0, "todayTotalTokens": 0, "todayTokensByModel": {},
            "recentDays": [], "totalPrompts": prompts, "totalSessions": 0,
            "activeDays": 0, "activeDates": [], "modelUsage": {}
        })
    }

    #[test]
    fn shadow_comparison_reports_only_field_names() {
        let comparison = compare_local_fields(&record(2), &record(1));
        assert_eq!(
            comparison.differing_fields,
            ["todayPrompts", "totalPrompts"]
        );
        let receipt = sanitized_receipt(&comparison).to_string();
        assert!(!receipt.contains('2'));
        assert!(!receipt.contains("prompt text"));
    }

    #[test]
    fn invalid_candidate_record_is_rejected() {
        assert!(validate_record(br#"{"schemaVersion":1}"#).is_err());
        assert!(validate_record(b"not json").is_err());
    }

    #[test]
    fn canary_requires_fingerprint_sources_flags_and_valid_rpc() {
        let valid = record(1);
        assert!(canary_eligible(true, false, false, &valid));
        assert!(!canary_eligible(false, false, false, &valid));
        assert!(!canary_eligible(true, true, false, &valid));
        assert!(!canary_eligible(true, false, true, &valid));
        let mut failed_rpc = valid;
        failed_rpc["usageStatusText"] = "Codex limits unavailable".into();
        assert!(!canary_eligible(true, false, false, &failed_rpc));
    }
}
