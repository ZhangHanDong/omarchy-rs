use std::path::Path;

use ccusage_adapter_codex::{CodexServiceTier as CcusageServiceTier, CodexTokenUsageEvent};

use crate::{CodexServiceTier, CodexUsageEvent};

pub(crate) fn load_codex_events_from_directory(
    sessions_dir: &Path,
) -> Result<Vec<CodexUsageEvent>, String> {
    ccusage_adapter_codex::load_codex_events_from_directory(sessions_dir, true)
        .map(|events| events.into_iter().map(map_event).collect())
        .map_err(|error| error.to_string())
}

fn map_event(event: CodexTokenUsageEvent) -> CodexUsageEvent {
    CodexUsageEvent {
        session_id: event.session_id,
        timestamp: event.timestamp,
        model: event.model,
        input_tokens: event.input_tokens,
        cached_input_tokens: event.cached_input_tokens,
        output_tokens: event.output_tokens,
        reasoning_output_tokens: event.reasoning_output_tokens,
        total_tokens: event.total_tokens,
        service_tier: event.service_tier.map(|tier| match tier {
            CcusageServiceTier::Standard => CodexServiceTier::Standard,
            CcusageServiceTier::Fast => CodexServiceTier::Fast,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_ccusage_adapter_parses_synthetic_codex_fixture() {
        let sessions = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/agent_usage/codex/valid");

        let events = load_codex_events_from_directory(&sessions)
            .expect("synthetic Codex fixture must parse");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].input_tokens, 120);
        assert_eq!(events[0].cached_input_tokens, 20);
        assert_eq!(events[0].output_tokens, 30);
        assert_eq!(events[0].total_tokens, 150);
    }
}
