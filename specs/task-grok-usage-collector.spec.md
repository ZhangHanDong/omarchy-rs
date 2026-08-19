spec: task
name: "Grok Agent Usage Collector"
inherits: project
tags: [agent-usage, grok, local-data]
satisfies: [REQ-001]
depends: [task-user-overlay-lifecycle]
estimate: 1d
---

<!-- lint-ack: verification-metadata-suggestion — Filesystem scenarios use synthetic TempDir session trees and never read real prompt content. -->

## Intent

Add Grok CLI to the Agent Usage panel by collecting its content-free local
turn usage in Rust. Read only structured completion metadata from Grok session
updates, preserve the common panel record schema, and keep malformed or future
records from crashing the updater.

## Decisions

- Scan `$GROK_HOME/sessions/**/updates.jsonl`, defaulting to `~/.grok`, with one reusable line buffer.
- Accept only `_x.ai/session/update` records whose update is `turn_completed` and whose usage has nonzero input or output tokens.
- Deduplicate records by `params._meta.eventId`; the last valid event with the same id wins.
- Aggregate `modelUsage` when present and otherwise assign top-level usage to the `grok` model bucket.
- Never persist or inspect prompt, response, tool argument, auth, or telemetry content.
- Ship `omarchy-agent-usage-grok` as a native Rust collector.

## Boundaries

### Allowed Changes
- Cargo.*
- crates/omarchy-agents/**
- crates/omarchy-cli/**
- crates/omarchy-compat/**
- fixtures/agent_usage/grok/**
- docs/components/agent-usage.md
- docs/deployment.md
- specs/task-grok-usage-collector.spec.md

### Forbidden
- Do not read `chat_history.jsonl`, `prompt_history.jsonl`, auth files, or telemetry logs.
- Do not enable Grok external OpenTelemetry.
- Do not fabricate usage, rate limits, balance, or subscription tier.
- Do not modify official Omarchy package files or the Grok installation.

## Completion Criteria

### Rule: grok-local-parity — Aggregate content-free completed turns

Scenario: Completed turns produce panel totals
  Test:
    Package: omarchy-agents
    Filter: grok_fixture_aggregates_completed_turns
  Given synthetic Grok updates with two sessions, dates, and model usage
  When the Rust collector scans the session tree
  Then prompt, session, date, input, output, cache, reasoning, and model totals match the fixture

Scenario: Malformed and duplicate updates are rejected safely
  Test:
    Package: omarchy-agents
    Filter: grok_malformed_and_duplicate_events
  Given malformed lines, unrelated updates, and a repeated event id
  When the Rust collector scans the session tree
  Then malformed records are skipped and the last valid duplicate contributes once

Scenario: Empty Grok home remains hidden
  Test:
    Package: omarchy-agents
    Filter: grok_empty_home_has_no_local_stats
  Given a Grok home with no completed usage events
  When the Rust collector creates a panel record
  Then it reports zero prompts, zero sessions, no limits, and `hasLocalStats=false`

### Rule: grok-native-routing — Route Grok through the managed overlay

Scenario: Activation enables the native Grok collector
  Test:
    Package: omarchy-cli
    Filter: activate_agent_usage_enables_grok
  Given a complete release sibling set including the native Grok collector
  When `activate agent-usage` writes the activation record
  Then Grok is enabled and status reports its current backend without requiring an upstream collector fingerprint

Scenario: Updater selection recognizes Grok
  Test:
    Package: omarchy-compat
    Filter: provider_selection_includes_grok
  Given an activated Grok provider and standard updater selection arguments
  When provider selection is evaluated
  Then Grok is selected unless explicitly excluded or another provider is the only requested id

## Out of Scope

- Grok account rate-limit or subscription APIs.
- External OpenTelemetry collection.
- Parsing prompt or response content.
- Modifying the stock Omarchy Agents plugin.
