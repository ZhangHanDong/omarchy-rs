spec: task
name: "Agent Usage Collector Parity Pilot"
inherits: project
tags: [rewrite, parity]
satisfies: [REQ-001, REQ-002]
depends: [task-agent-usage-dependency-evaluation]
estimate: 3w
---

<!-- lint-ack: output-mode-coverage — The collector's only machine output is the state file, covered by codex_state_file_output_parity; it has no -o/--output mode. -->

## Intent

Implement the first omarchy-rs replacement for the read-only Agent Usage
collectors. Match the installed Omarchy collectors on versioned synthetic
fixtures, reduce repeated scanning and helper-process creation, and prove that
activation and rollback do not affect official Omarchy upgrades.

## Decisions

- Treat the checked-out `../omarchy/bin/omarchy-agent-usage-*` scripts as the fixture compatibility baseline.
- Parse Codex logs through ZhangHanDong/ccusage revision `9f6c0305743f29c99dbfa2ade54065e66632a2bb` behind an internal backend.
- Start with Codex; add Claude and Fireworks only after the shared contract passes.
- Keep the upstream JSON state schema and atomic file replacement semantics.
- Resolve fallback through an absolute upstream executable path to prevent recursion.
- Use synthetic local fixtures; tests and benchmarks perform no network access and read no real HOME state.
- Primary performance metrics are CPU time and bytes read for an unchanged warm fixture; numeric activation thresholds remain unresolved until the baseline harness is run.

## Boundaries

### Allowed Changes
- crates/omarchy-cli/**
- crates/omarchy-agents/**
- crates/omarchy-compat/**
- tests/agent_usage/**
- benches/agent_usage/**
- packaging/**
- docs/components/agent-usage.md
- knowledge/**
- specs/task-agent-usage-parity.spec.md

### Forbidden
- Do not leave compatibility requirements as prose-only notes
- Do not replace current user-visible behavior unless this task explicitly changes the contract
- Do not read the developer's real agent logs, credentials, or HOME in tests.
- Do not activate a shim when the precedence probe or compatibility check fails.

## Completion Criteria

### Rule: collector-parity — Preserve collector behavior

Scenario: Codex collector keeps JSON and exit-status parity
  Test:
    Package: omarchy-agents
    Filter: codex_fixture_parity
    Level: cli
    Test Double: synthetic_agent_home
    Targets: crates/omarchy-agents/src/codex.rs, tests/agent_usage/codex.rs
  Given versioned Codex session fixtures and an isolated HOME
  When upstream and Rust collectors receive identical arguments and environment
  Then their normalized JSON state, stdout, stderr, and exit status are equivalent

Scenario: Malformed Codex records preserve partial-failure behavior
  Test:
    Package: omarchy-agents
    Filter: codex_malformed_record_parity
    Level: integration
    Test Double: malformed_synthetic_agent_home
    Targets: crates/omarchy-agents/src/codex.rs, tests/agent_usage/codex.rs
  Given valid and malformed records in the same synthetic Codex session tree
  When upstream and Rust collectors scan the fixture
  Then skipped records, persisted state, diagnostics, and exit status are equivalent

Scenario: Collector state file keeps schema and atomic replacement parity
  Test:
    Package: omarchy-agents
    Filter: codex_state_file_output_parity
    Level: integration
    Test Double: synthetic_agent_home
    Targets: crates/omarchy-agents/src/codex.rs, tests/agent_usage/codex.rs
  Given an existing state file and versioned Codex session fixtures
  When the Rust collector refreshes the state file
  Then the resulting JSON fields and permissions match upstream behavior
  And readers observe either the complete old file or the complete new file

### Rule: overlay-safety — Activate and roll back safely

Scenario: Compatible shim resolves to Rust and rolls back offline
  Test:
    Package: omarchy-compat
    Filter: shim_activation_and_offline_rollback
    Level: integration
    Test Double: isolated_path_and_fake_upstream
    Targets: crates/omarchy-compat/src/overlay.rs, tests/agent_usage/overlay.rs
  Given a compatible fake upstream executable and a PATH where the overlay precedes it
  When the replacement is activated and then disabled without network access
  Then activation resolves the Rust collector and rollback resolves the absolute upstream executable

Scenario: Drift or invalid precedence refuses activation
  Test:
    Package: omarchy-compat
    Filter: activation_rejects_drift_and_bad_precedence
    Level: integration
    Test Double: isolated_path_and_changed_upstream
    Targets: crates/omarchy-compat/src/overlay.rs, tests/agent_usage/overlay.rs
  Given either an unverified upstream fingerprint or overlay path after upstream path
  When activation is requested
  Then activation fails without changing official files or existing shims

### Rule: measured-value — Require evidence before default activation

Scenario: Warm fixture benchmark emits comparable metrics
  Mode: optimize
  Test:
    Package: omarchy-agents
    Filter: benchmark_report_contains_required_metrics
    Level: integration
    Test Double: versioned_warm_fixture
    Targets: benches/agent_usage/codex.rs, tests/agent_usage/benchmark_report.rs
  Given identical warm fixtures, environment metadata, warm-up, and sample count
  When upstream and Rust collectors are benchmarked
  Then the report contains wall time, CPU time, maximum RSS, available I/O bytes, and child-process count for both implementations

Scenario: Missing improvement or resource regression prevents default activation
  Test:
    Package: omarchy-compat
    Filter: eligibility_requires_performance_gate
    Level: integration
    Test Double: benchmark_reports
    Targets: crates/omarchy-compat/src/eligibility.rs, tests/agent_usage/eligibility.rs
  Given a report with no primary-metric improvement or a declared resource regression
  When default-activation eligibility is evaluated
  Then the replacement remains disabled by default

Scenario: Benchmark report excludes sensitive fixture markers
  Test:
    Package: omarchy-agents
    Filter: benchmark_report_redacts_sensitive_markers
    Level: integration
    Test Double: sensitive_marker_fixture
    Targets: benches/agent_usage/codex.rs, tests/agent_usage/benchmark_report.rs
  Given synthetic fixtures containing prompt and credential markers
  When a benchmark report is persisted
  Then no sensitive marker occurs in the report

## Out of Scope

- Claude and Fireworks implementation until the Codex rule passes.
- A top-level `omarchy` proxy.
- A resident daemon.
- Mutating or privileged Omarchy commands.

## Questions

- What CPU-time or bytes-read improvement is sufficient for default activation?
- [x] First fixture baseline: Omarchy `f32ebbdb730c4e8fe11e4046cef4267e466264ea`.
