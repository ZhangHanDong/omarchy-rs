use std::path::Path;

use ccusage_adapter_codex::{
    CodexDirectoryLoadOptions, CodexServiceTier as CcusageServiceTier, CodexTokenUsageEvent,
    load_codex_events_from_directory_with_options,
};

use crate::{CodexServiceTier, CodexUsageEvent};

pub(crate) fn load_codex_events_from_directory(
    sessions_dir: &Path,
) -> Result<Vec<CodexUsageEvent>, String> {
    load_codex_events_from_directory_with_options(
        sessions_dir,
        CodexDirectoryLoadOptions {
            single_thread: true,
            deduplicate: false,
            filter_replayed_events: false,
            filter_unchanged_cumulative_events: false,
        },
    )
    .map(|events| events.into_iter().map(map_event).collect())
    .map_err(|error| error.to_string())
}

fn map_event(event: CodexTokenUsageEvent) -> CodexUsageEvent {
    let model = (!event.is_fallback_model).then_some(event.model).flatten();
    CodexUsageEvent {
        session_id: event.session_id,
        timestamp: event.timestamp,
        model,
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
        assert_eq!(events[0].model, None);
    }
}
