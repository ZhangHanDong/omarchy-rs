pub mod benchmark;
mod ccusage_backend;
pub mod claude;
pub mod codex;
pub mod octoscode;
pub mod rpc;

use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexServiceTier {
    Standard,
    Fast,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexUsageEvent {
    pub session_id: String,
    pub timestamp: String,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
    pub service_tier: Option<CodexServiceTier>,
}

pub fn load_codex_events_from_directory(
    sessions_dir: &Path,
) -> Result<Vec<CodexUsageEvent>, String> {
    ccusage_backend::load_codex_events_from_directory(sessions_dir)
}
