#[path = "../crates/omarchy-compat/src/activation.rs"]
pub mod activation;
#[path = "../crates/omarchy-agents/src/benchmark.rs"]
pub mod benchmark;
#[path = "../crates/omarchy-agents/src/claude.rs"]
pub mod claude;
#[path = "../crates/omarchy-cleaner/src/lib.rs"]
pub mod cleaner;
#[path = "../crates/omarchy-agents/src/codex.rs"]
pub mod codex;
#[path = "../crates/omarchy-agents/src/grok.rs"]
pub mod grok;
#[path = "../crates/omarchy-learn/src/lib.rs"]
pub mod learn;
#[path = "../crates/omarchy-network/src/lib.rs"]
pub mod network;
#[path = "../crates/omarchy-agents/src/octoscode.rs"]
pub mod octoscode;
#[path = "../crates/omarchy-plugins/src/lib.rs"]
pub mod plugins;
#[path = "../crates/omarchy-agents/src/rpc.rs"]
pub mod rpc;
#[path = "../crates/omarchy-compat/src/shadow.rs"]
pub mod shadow;
#[path = "../crates/omarchy-skills/src/lib.rs"]
pub mod skills;

#[path = "../crates/omarchy-cli/src/lib.rs"]
mod cli;
mod codex_adapter;
mod release;

pub use cli::{Command, Layout, execute};

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
    codex_adapter::load_codex_events_from_directory(sessions_dir)
}
