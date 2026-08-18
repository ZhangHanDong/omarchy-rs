use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CcusageForkRecord {
    pub schema_version: u32,
    pub source: String,
    pub upstream: String,
    pub branch: String,
    pub upstream_revision: String,
    pub patch_revision: String,
    pub dependency_revision: String,
    pub features: Vec<String>,
    pub offline_build: String,
    pub default_missing_pricing: String,
    pub changed_paths: Vec<String>,
    pub patch_commits: Vec<String>,
}

pub fn validate_ccusage_fork_record(record: &CcusageForkRecord) -> bool {
    record.schema_version == 1
        && is_full_git_revision(&record.upstream_revision)
        && is_full_git_revision(&record.patch_revision)
        && record.patch_revision == record.dependency_revision
        && record.source == "https://github.com/ZhangHanDong/ccusage"
        && record.upstream == "https://github.com/ccusage/ccusage"
        && record.branch == "omarchy-rs"
        && record.features == ["models-dev-pricing-only"]
        && !record.offline_build.trim().is_empty()
        && !record.default_missing_pricing.trim().is_empty()
        && !record.changed_paths.is_empty()
        && !record.patch_commits.is_empty()
}

fn is_full_git_revision(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Accept,
    Adapt,
    Isolate,
    Reject,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateRecord {
    pub name: String,
    pub version: String,
    pub source: String,
    pub source_revision: String,
    pub license: String,
    pub maintenance: String,
    pub msrv: String,
    pub default_features: Vec<String>,
    pub proposed_features: Vec<String>,
    pub transitive_dependencies: String,
    pub build_script: String,
    pub unsafe_findings: String,
    pub credential_access: String,
    pub network_access: String,
    pub telemetry: String,
    pub advisory_status: String,
    pub behavior_coverage: Vec<String>,
    pub behavior_gaps: Vec<String>,
    pub evidence: Vec<String>,
    pub outcome: Outcome,
    pub rationale: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionReport {
    pub schema_version: u32,
    pub evaluated_at: String,
    pub omarchy_baseline: String,
    pub candidates: Vec<CandidateRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    MissingCandidate,
    MissingEvidence,
    UnsafeDefault,
    DeniedPolicy,
    DirectAcceptWithGaps,
}

pub fn parse_report(input: &str) -> Result<AdmissionReport, serde_json::Error> {
    serde_json::from_str(input)
}

pub fn validate_report(report: &AdmissionReport) -> Result<(), ValidationError> {
    for required in ["tokenusage", "ccusage-rust-adapters", "claude-usage"] {
        if !report
            .candidates
            .iter()
            .any(|candidate| candidate.name == required)
        {
            return Err(ValidationError::MissingCandidate);
        }
    }

    for candidate in &report.candidates {
        let required_text = [
            &candidate.version,
            &candidate.source,
            &candidate.source_revision,
            &candidate.license,
            &candidate.maintenance,
            &candidate.msrv,
            &candidate.transitive_dependencies,
            &candidate.build_script,
            &candidate.unsafe_findings,
            &candidate.credential_access,
            &candidate.network_access,
            &candidate.telemetry,
            &candidate.advisory_status,
            &candidate.rationale,
        ];
        if required_text.iter().any(|value| value.trim().is_empty())
            || candidate.evidence.is_empty()
            || candidate.behavior_coverage.is_empty()
        {
            return Err(ValidationError::MissingEvidence);
        }

        let exposes_sensitive_boundary =
            candidate.credential_access != "none" || candidate.network_access != "none";
        if exposes_sensitive_boundary && candidate.outcome == Outcome::Accept {
            return Err(ValidationError::UnsafeDefault);
        }
        if (candidate.advisory_status.starts_with("deny:")
            || !matches!(candidate.license.as_str(), "MIT" | "MIT OR Apache-2.0"))
            && candidate.outcome != Outcome::Reject
        {
            return Err(ValidationError::DeniedPolicy);
        }
        if !candidate.behavior_gaps.is_empty() && candidate.outcome == Outcome::Accept {
            return Err(ValidationError::DirectAcceptWithGaps);
        }
    }
    Ok(())
}

pub fn validate_fixture_root(workspace: &Path, fixture_root: &Path) -> bool {
    let allowed = workspace.join("fixtures/agent_usage");
    match (allowed.canonicalize(), fixture_root.canonicalize()) {
        (Ok(allowed), Ok(fixture_root)) => fixture_root.starts_with(allowed),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    const REAL_HOME_CANARY: &str = "OMARCHY_RS_REAL_HOME_CANARY_DO_NOT_READ";

    fn workspace() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .components()
            .collect()
    }

    fn report_text() -> String {
        fs::read_to_string(workspace().join("docs/dependencies/agent-usage.json"))
            .expect("dependency admission report must exist")
    }

    fn report() -> AdmissionReport {
        parse_report(&report_text()).expect("dependency admission report must parse")
    }

    fn ccusage_fork_record() -> CcusageForkRecord {
        let text = fs::read_to_string(workspace().join("docs/dependencies/ccusage-fork.json"))
            .expect("ccusage fork record must exist");
        serde_json::from_str(&text).expect("ccusage fork record must parse")
    }

    #[test]
    fn ccusage_fork_record_pins_upstream_and_patch_revisions() {
        let record = ccusage_fork_record();
        assert!(validate_ccusage_fork_record(&record));

        let manifest = fs::read_to_string(workspace().join("crates/omarchy-agents/Cargo.toml"))
            .expect("production agent manifest exists");
        assert!(manifest.contains(&format!("rev = \"{}\"", record.patch_revision)));
        assert!(!manifest.contains("branch = \"omarchy-rs\""));
    }

    #[test]
    fn ccusage_fork_record_rejects_moving_dependency() {
        let mut record = ccusage_fork_record();
        record.dependency_revision.clear();
        assert!(!validate_ccusage_fork_record(&record));
    }

    #[test]
    fn ccusage_models_dev_only_builds_offline() {
        let record = ccusage_fork_record();
        assert_eq!(record.features, ["models-dev-pricing-only"]);
        assert!(record.offline_build.contains("--offline"));
        assert!(
            !record
                .features
                .iter()
                .any(|feature| feature.contains("fetch"))
        );
    }

    #[test]
    fn ccusage_default_missing_pricing_fails_closed() {
        let record = ccusage_fork_record();
        assert!(record.default_missing_pricing.contains("fails closed"));
        assert!(record.default_missing_pricing.contains("does not enable"));
    }

    #[test]
    fn ccusage_fork_excludes_omarchy_specific_behavior() {
        let record = ccusage_fork_record();
        assert!(record.changed_paths.iter().all(|path| {
            path == "docs/omarchy-rs-fork.md"
                || path.starts_with("rust/crates/ccusage-core/")
                || path == "rust/adapters/codex/src/lib.rs"
                || path == "rust/adapters/codex/src/loader.rs"
                || (path.starts_with("rust/") && path.ends_with("Cargo.toml"))
        }));

        let forbidden = [
            "state",
            "cache",
            "app-server",
            "overlay",
            "activation",
            "rollback",
        ];
        for path in record.changed_paths {
            assert!(!forbidden.iter().any(|term| path.contains(term)));
        }
    }

    #[test]
    fn dependency_records_include_required_fields() {
        let report = report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.omarchy_baseline.len(), 40);
        assert_eq!(report.candidates.len(), 3);
        assert_eq!(validate_report(&report), Ok(()));
    }

    #[test]
    fn incomplete_dependency_record_is_rejected() {
        let mut report = report();
        report.candidates[0].license.clear();
        assert_eq!(
            validate_report(&report),
            Err(ValidationError::MissingEvidence)
        );
    }

    #[test]
    fn dependency_probe_uses_only_synthetic_homes() {
        let codex_root = workspace().join("fixtures/agent_usage/codex/valid");
        let claude_root = workspace().join("fixtures/agent_usage/claude/valid");
        assert!(validate_fixture_root(&workspace(), &codex_root));
        assert!(validate_fixture_root(&workspace(), &claude_root));
        assert!(!validate_fixture_root(
            &workspace(),
            &PathBuf::from("/home/alexzhang/.codex")
        ));

        for fixture in [
            codex_root.join("session.jsonl"),
            claude_root.join("transcript.jsonl"),
        ] {
            let fixture = fs::read_to_string(fixture).expect("fixture exists");
            assert!(!fixture.contains(REAL_HOME_CANARY));
        }
        assert!(!report_text().contains(REAL_HOME_CANARY));
    }

    #[test]
    fn credential_or_network_candidate_cannot_be_default() {
        let report = report();
        for candidate in report.candidates {
            if candidate.credential_access != "none" || candidate.network_access != "none" {
                assert!(matches!(
                    candidate.outcome,
                    Outcome::Adapt | Outcome::Isolate | Outcome::Reject
                ));
            }
        }
    }

    #[test]
    fn candidate_coverage_matrix_matches_fixtures() {
        let report = report();
        let fixture_classes = ["valid", "empty", "malformed", "duplicate", "cold", "warm"];
        for provider in ["codex", "claude"] {
            for class in fixture_classes {
                assert!(
                    workspace()
                        .join("fixtures/agent_usage")
                        .join(provider)
                        .join(class)
                        .exists(),
                    "missing {provider}/{class} fixture"
                );
            }
        }
        for candidate in report.candidates {
            assert!(!candidate.behavior_coverage.is_empty());
            assert!(!candidate.behavior_gaps.is_empty());
        }
    }

    #[test]
    fn missing_omarchy_behavior_prevents_direct_acceptance() {
        let report = report();
        assert!(report.candidates.iter().all(|candidate| {
            candidate.behavior_gaps.is_empty() || candidate.outcome != Outcome::Accept
        }));
    }

    #[test]
    fn denied_advisory_or_license_rejects_candidate() {
        let mut report = report();
        report.candidates[0].advisory_status = "deny:RUSTSEC-TEST".into();
        assert_eq!(validate_report(&report), Err(ValidationError::DeniedPolicy));
        report.candidates[0].outcome = Outcome::Reject;
        assert_eq!(validate_report(&report), Ok(()));
    }

    #[test]
    fn accepted_candidate_disables_unrelated_features() {
        let report = report();
        for candidate in report.candidates {
            if matches!(candidate.outcome, Outcome::Accept | Outcome::Adapt) {
                for forbidden in ["cli", "tui", "gui", "image", "telemetry", "network"] {
                    assert!(
                        !candidate
                            .proposed_features
                            .iter()
                            .any(|feature| feature == forbidden)
                    );
                }
            }
        }
    }
}
