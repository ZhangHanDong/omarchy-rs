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
- Parse Codex logs through ZhangHanDong/ccusage revision `97f5b4e71864408c4df5a9758639d253caf57dce` behind an internal backend.
- Start with Codex; add Claude and Fireworks only after the shared contract passes.
- Keep the upstream JSON state schema and atomic file replacement semantics.
- Resolve fallback through an absolute upstream executable path to prevent recursion.
- Use synthetic local fixtures; tests and benchmarks perform no network access and read no real HOME state.
- Primary performance metrics are CPU time and bytes read for an unchanged warm fixture; the activation thresholds below are fixed before candidate measurement.
- Default activation requires at least 30% lower CPU time and 20% lower p95 wall time, with no more than 20% higher maximum RSS or 10% more bytes read.
- A strong replacement claim requires either 50% lower CPU time, 2x wall-time speedup, or 40% lower measured daily CPU work at the upstream refresh interval.
- Stability admission requires 1,000 repeated valid runs without crash or panic, deterministic normalized output, and no partial JSON during concurrent-reader atomic-write stress.
- Energy improvement is claimed only from package-energy measurements; CPU-time reduction is labeled an energy proxy.

## Boundaries

### Allowed Changes
- Cargo.toml
- crates/omarchy-cli/**
- crates/omarchy-agents/**
- crates/omarchy-compat/**
- tests/agent_usage/**
- benches/agent_usage/**
- packaging/**
- docs/components/agent-usage.md
- docs/benchmarks/**
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

Scenario: Codex app-server limits preserve RPC field parity
  Test:
    Package: omarchy-agents
    Filter: codex_rpc_fake_app_server_round_trip
    Level: integration
    Test Double: fake_codex_app_server
    Targets: crates/omarchy-agents/src/rpc.rs
  Given a fake Codex app-server that returns account and primary and secondary rate-limit windows
  When the Rust collector completes initialize, account/read, and account/rateLimits/read
  Then limits, tierLabel, usageStatusText, and authHelpText match the upstream JSON shape
  And no provider network or credential store is accessed

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

Scenario: Shadow mode preserves upstream output while recording local parity
  Test:
    Package: omarchy-compat
    Filter: shadow_mode_preserves_upstream_output
    Level: integration
    Test Double: isolated_path_and_fake_upstream
    Targets: crates/omarchy-compat/src/shadow.rs, crates/omarchy-compat/src/bin/omarchy-agent-usage-codex-shadow.rs
  Given isolated synthetic Codex sessions and a verified absolute fake upstream collector
  When the shadow collector runs with the same arguments and environment
  Then stdout and exit status come from upstream even if the Rust candidate fails or differs
  And the parity receipt contains field names but no usage values, credentials, or prompt content

Scenario: Canary mode fails open on every unverified compatibility surface
  Test:
    Package: omarchy-compat
    Filter: canary_requires_fingerprint_sources_flags_and_valid_rpc
    Level: unit
    Test Double: compatibility_inputs
    Targets: crates/omarchy-compat/src/shadow.rs
  Given the upstream fingerprint, external-source presence, requested flags, and Rust RPC status
  When canary eligibility is evaluated
  Then Rust is selected only for the verified fingerprint without Pi, OMP, OpenCode, limits-only, or RPC failure
  And every other combination selects the absolute Python fallback

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

Scenario: Numeric performance gate controls eligibility
  Test:
    Package: omarchy-compat
    Filter: eligibility_enforces_codex_numeric_thresholds
    Level: unit
    Test Double: benchmark_reports
    Targets: crates/omarchy-compat/src/eligibility.rs, tests/agent_usage/eligibility.rs
  Given a parity-passing report with candidate and upstream confidence intervals
  When Codex default-activation eligibility is evaluated
  Then CPU time is at least 30% lower and p95 wall time is at least 20% lower
  And maximum RSS is no more than 20% higher and bytes read are no more than 10% higher

Scenario: Repeated runs and concurrent reads remain stable
  Mode: optimize
  Test:
    Package: omarchy-agents
    Filter: stability_report_meets_codex_gate
    Level: integration
    Test Double: synthetic_valid_malformed_and_atomic_write_fixtures
    Targets: benches/agent_usage/codex.rs, tests/agent_usage/stability_report.rs
  Given 1,000 repeated valid and malformed runs plus concurrent state-file readers
  When the stability campaign completes
  Then crash and panic counts are zero and normalized valid outputs are deterministic
  And every observed state file is one complete valid JSON document

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

- [x] Default activation gate: CPU -30%, p95 wall -20%, RSS <= +20%, bytes read <= +10%.
- [x] Stability gate: 1,000 runs, zero crash/panic, deterministic normalized output, atomic JSON only.
- [x] First fixture baseline: Omarchy `f32ebbdb730c4e8fe11e4046cef4267e466264ea`.
