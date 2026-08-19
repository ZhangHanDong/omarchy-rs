use std::{env, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

pub const PROVIDERS: [&str; 4] = ["codex", "claude", "octoscode", "grok"];
pub const CODEX_UPSTREAM_SHA256: &str =
    "0d36d856439f17749dc8a25c56607e8462de72fde91f384abc370fbc78113b14";
pub const CLAUDE_UPSTREAM_SHA256: &str =
    "88938e35170a5ef8da30b665114740440eb3bd68dba9579a818e8b02c4a0ffc3";
pub const OCTOSCODE_UPSTREAM_SHA256: &str =
    "d67554a97fd4c27bec3c1557f06fba4498aaebe949eb8836d7c145ce9a9b707a";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationConfig {
    pub schema_version: u32,
    pub component: String,
    pub providers: Vec<String>,
}

impl ActivationConfig {
    pub fn agent_usage() -> Self {
        Self {
            schema_version: 1,
            component: "agent-usage".into(),
            providers: PROVIDERS.into_iter().map(str::to_string).collect(),
        }
    }

    pub fn enables(&self, provider: &str) -> bool {
        self.schema_version == 1
            && self.component == "agent-usage"
            && self.providers.iter().any(|enabled| enabled == provider)
    }
}

pub fn activation_path() -> Result<PathBuf, String> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|root| root.join("omarchy-rs/activation.json"))
        .ok_or_else(|| "HOME and XDG_CONFIG_HOME are unset".into())
}

pub fn load_activation() -> Option<ActivationConfig> {
    fs::read(activation_path().ok()?)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}
