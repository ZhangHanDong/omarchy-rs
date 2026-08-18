spec: task
name: "Claude Usage Collector Parity Canary"
inherits: project
tags: [rewrite, parity, canary]
satisfies: [REQ-001, REQ-002]
depends: [task-agent-usage-parity]
estimate: 1w
---

## Intent

Add a Rust Claude Code usage collector behind the existing reversible Agent
Usage overlay. Preserve the installed Python collector as the fail-open
baseline while moving native Claude transcript aggregation and authenticated
limits collection onto Rust for verified installations.

## Decisions

- Discover native Claude Code transcripts through the pinned `ccusage-adapter-claude` fork revision, then apply an Omarchy compatibility parser because ccusage's richer advisor/iteration aggregation is not state-file compatible.
- Preserve the upstream state-file JSON fields and add only `collectorBackend` for UI provenance.
- Read credentials only to send the OAuth access token to Anthropic's fixed usage endpoint; never persist or log the token.
- Select Rust only for the verified upstream fingerprint and native Claude sources; Pi, OMP, OpenCode, unsupported flags, network/parser failure, or fingerprint drift select the absolute Python fallback.
- Keep the overlay update command provider-selective so Codex and Claude can independently use or roll back their Rust collectors.
- Use synthetic fixtures and fake local HTTP responses; tests never read the developer's HOME, credentials, prompts, or network.

## Boundaries

### Allowed Changes
- Cargo.*
- crates/omarchy-agents/**
- crates/omarchy-compat/**
- crates/dependency-probe/**
- fixtures/agent_usage/claude/**
- docs/components/agent-usage.md
- docs/benchmarks/**
- docs/dependencies/**
- specs/task-claude-usage-parity.spec.md
- specs/project.spec.md

### Forbidden
- Do not modify `../omarchy/**` or package-owned Omarchy paths.
- Do not log or persist Claude OAuth access tokens or prompt contents.
- Do not select Rust when an input surface lacks differential coverage.

## Completion Criteria

### Rule: claude-local-parity — Preserve native Claude statistics

Scenario: Native Claude fixture produces the upstream state shape
  Test:
    Package: omarchy-agents
    Filter: claude_fixture_parity
  Given a versioned native Claude transcript under an isolated config directory
  When the Rust collector scans the fixture
  Then normalized token, prompt, session, date, and model fields equal the Python baseline

Scenario: Malformed and duplicate transcript records remain bounded
  Test:
    Package: omarchy-agents
    Filter: claude_malformed_and_duplicate_records
  Given malformed lines and duplicate assistant usage entries
  When the Rust collector scans the synthetic files
  Then malformed lines are skipped and duplicate usage is counted once without panic

Scenario: Aggregate fallback is used only without transcript usage
  Test:
    Package: omarchy-agents
    Filter: claude_stats_cache_and_history_fallback
  Given an isolated Claude directory with no usable transcript usage and synthetic aggregate files
  When the Rust collector builds local statistics
  Then stats-cache supplies aggregate totals and history supplies today's prompt and session counts

### Rule: claude-limits-safety — Keep credentials and failures safe

Scenario: OAuth limit payload maps to the panel schema
  Test:
    Package: omarchy-agents
    Filter: claude_limits_payload_parity
  Given a synthetic Anthropic payload with session, weekly, and scoped model limits
  When Rust normalizes its percentages and reset timestamps
  Then the limits and tier label equal the upstream JSON shape
  And no access token occurs in output or diagnostics

Scenario: Missing or expired credentials return local-only status
  Test:
    Package: omarchy-agents
    Filter: claude_missing_and_expired_credentials
    Level: integration
    Test Double: isolated_credentials_and_forbidden_network
  Given missing or expired synthetic credentials
  When the Rust collector requests a record
  Then it performs no network request and returns the upstream status and help fields

### Rule: claude-overlay-safety — Select and roll back independently

Scenario: Verified native Claude input selects Rust
  Test:
    Package: omarchy-compat
    Filter: claude_canary_selects_verified_rust
  Given the verified upstream fingerprint, native Claude sources, supported flags, and a valid Rust record
  When the Claude canary runs
  Then it emits `collectorBackend=rust` and writes a complete atomic `claude.json`

Scenario: Unverified Claude input fails open to Python
  Test:
    Package: omarchy-compat
    Filter: claude_canary_falls_back_for_unverified_surfaces
  Given fingerprint drift, Pi, OMP, OpenCode, unsupported flags, or a Rust collector failure
  When the Claude canary runs
  Then it invokes the absolute upstream collector and emits `collectorBackend=python`
  And Codex selection is unchanged

## Out of Scope

- Rust parsing of Pi, OMP, or OpenCode Claude usage in this canary.
- Replacing the Claude CLI or authentication flow.
- Enabling Claude Rust by default before parity and benchmark evidence passes.

## Questions

- [x] External Claude consumers remain on the Python fail-open path until separately covered.
