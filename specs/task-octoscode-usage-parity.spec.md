spec: task
name: "Octoscode Usage Collector Parity Canary"
inherits: project
tags: [rewrite, parity, canary]
satisfies: [REQ-001, REQ-002]
depends: [task-claude-usage-parity]
estimate: 2d
---

<!-- lint-ack: output-mode-coverage — The only file-output mode is `--write`, covered by the atomic state-file scenario. -->

## Intent

Replace the user-owned Octoscode Agent Usage collector's ledger scan with a
Rust canary while retaining the existing Python script as an absolute
fail-open fallback. Preserve panel state fields and atomic `--write` behavior.

## Decisions

- Parse only `instances/*/ui-protocol/**/ledger-*.log` files and preserve sorted file order and last-event-per-turn semantics.
- Associate `token_cost_update` model metadata with later `turn_completed` events by turn id.
- Select Rust only when the user-owned Python baseline has the verified fingerprint; drift or candidate failure selects Python.
- Keep the Python collector file unchanged and point only the user systemd service at the omarchy-rs wrapper.
- Add `collectorBackend` after record validation and atomically write the user state file.
- Stream ledger bytes through one reusable line buffer; never retain a complete ledger file, and preserve Python's replacement behavior for invalid UTF-8.
- Tests use synthetic ledgers and isolated state directories with no prompt content.

## Boundaries

### Allowed Changes
- Cargo.*
- crates/omarchy-agents/**
- crates/omarchy-compat/**
- fixtures/agent_usage/octoscode/**
- docs/components/agent-usage.md
- docs/benchmarks/**
- specs/task-octoscode-usage-parity.spec.md

### Forbidden
- Do not modify or delete the existing Python collector.
- Do not read real Octoscode ledger content in tests or persisted benchmark reports.
- Do not select Rust after baseline fingerprint drift.

## Completion Criteria

### Rule: octoscode-parity — Preserve ledger aggregation

Scenario: Token and model totals match the Python record
  Test:
    Package: omarchy-agents
    Filter: octoscode_fixture_parity
  Given a synthetic ledger with model metadata and completed turns
  When Rust collects the Octoscode record
  Then prompt, session, date, model, input, and output fields equal the Python baseline

Scenario: Malformed and repeated turns remain bounded
  Test:
    Package: omarchy-agents
    Filter: octoscode_malformed_and_repeated_turns
  Given malformed lines and repeated completion events for one turn
  When Rust scans the ledger
  Then malformed lines are skipped and the last valid completion replaces the earlier turn

Scenario: Large ledgers are streamed with invalid UTF-8 tolerance
  Test:
    Package: omarchy-agents
    Filter: octoscode_streams_large_ledgers
  Given a ledger much larger than its individual lines and containing invalid UTF-8
  When Rust scans the ledger through its reusable line buffer
  Then the valid completion is collected without retaining a whole-file text buffer

### Rule: octoscode-overlay — Preserve fallback and writes

Scenario: Verified canary selects Rust
  Test:
    Package: omarchy-compat
    Filter: octoscode_canary_selects_verified_rust
  Given a verified baseline fingerprint and valid Rust record
  When canary eligibility is evaluated
  Then Rust is selected independently from Claude and Codex

Scenario: Drift and invalid records select Python
  Test:
    Package: omarchy-compat
    Filter: octoscode_canary_falls_back
  Given fingerprint drift or an invalid Rust record
  When canary eligibility is evaluated
  Then the absolute Python fallback is selected

Scenario: File output uses complete JSON replacements
  Test:
    Package: omarchy-compat
    Filter: octoscode_state_write_is_atomic
  Given an existing Octoscode state file in an isolated state directory
  When the wrapper writes file output as a replacement record
  Then readers observe a complete valid record with the selected backend

## Out of Scope

- Modifying Octoscode itself or its ledger schema.
- Installing a system-wide service or collector.
- Default activation on machines with a different Python collector fingerprint.
