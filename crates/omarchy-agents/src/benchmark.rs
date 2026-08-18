use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BenchmarkSample {
    pub wall_ns: u128,
    pub user_seconds: f64,
    pub system_seconds: f64,
    pub max_rss_kib: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub voluntary_context_switches: u64,
    pub involuntary_context_switches: u64,
    pub child_process_count: u64,
    pub exit_code: i32,
    pub normalized_output_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BenchmarkReport {
    pub schema_version: u32,
    pub implementation: String,
    pub executable: String,
    pub fixture: String,
    pub fixture_sessions: usize,
    pub warmups: usize,
    pub measured_samples: usize,
    pub logical_cache_state: String,
    pub samples: Vec<BenchmarkSample>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BenchmarkEnvironment {
    pub captured_at_utc: String,
    pub os_release: String,
    pub kernel_release: String,
    pub cpu_model: String,
    pub clock_ticks_per_second: u64,
    pub upstream_sha256: String,
    pub candidate_sha256: String,
    pub omarchy_rs_commit: String,
    pub ccusage_commit: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AbBenchmarkReport {
    pub schema_version: u32,
    pub environment: BenchmarkEnvironment,
    pub execution_order: String,
    pub output_hashes_match: bool,
    pub upstream: BenchmarkReport,
    pub candidate: BenchmarkReport,
}

impl AbBenchmarkReport {
    pub fn validate(&self) -> Result<(), &'static str> {
        self.upstream.validate()?;
        self.candidate.validate()?;
        if self.schema_version != 1
            || self.upstream.measured_samples != self.candidate.measured_samples
            || self.upstream.fixture_sessions != self.candidate.fixture_sessions
        {
            return Err("A/B report shape mismatch");
        }
        if !self.output_hashes_match {
            return Err("normalized outputs differ");
        }
        Ok(())
    }
}

impl BenchmarkReport {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != 1 || self.samples.len() != self.measured_samples {
            return Err("sample count mismatch");
        }
        if self.samples.is_empty() {
            return Err("no measured samples");
        }
        if self.samples.iter().any(|sample| {
            sample.wall_ns == 0
                || sample.max_rss_kib == 0
                || sample.exit_code != 0
                || sample.normalized_output_sha256.len() != 64
        }) {
            return Err("invalid sample");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_report_contains_required_metrics() {
        let report = BenchmarkReport {
            schema_version: 1,
            implementation: "upstream".into(),
            executable: "/synthetic/upstream".into(),
            fixture: "synthetic".into(),
            fixture_sessions: 1,
            warmups: 1,
            measured_samples: 1,
            logical_cache_state: "cold".into(),
            samples: vec![BenchmarkSample {
                wall_ns: 1,
                user_seconds: 0.01,
                system_seconds: 0.01,
                max_rss_kib: 1,
                read_bytes: 0,
                write_bytes: 0,
                voluntary_context_switches: 0,
                involuntary_context_switches: 0,
                child_process_count: 0,
                exit_code: 0,
                normalized_output_sha256: "0".repeat(64),
            }],
        };
        assert_eq!(report.validate(), Ok(()));
    }
}
