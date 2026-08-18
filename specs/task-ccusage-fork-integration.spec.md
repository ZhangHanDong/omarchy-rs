spec: task
name: "Pinned ccusage Fork Integration"
inherits: project
tags: [dependencies, fork, agent-usage]
satisfies: [REQ-003]
depends: [task-agent-usage-dependency-evaluation]
estimate: 2d
---

## Intent

Adopt the ZhangHanDong ccusage fork immediately as the local Agent Usage parser
basis while keeping the fork as a small, replayable patch stack over upstream.
Make ordinary Cargo builds deterministic and offline, pin every consumed source
revision, and keep all Omarchy-specific behavior inside omarchy-rs.

## Decisions

- Keep fork `main` as an upstream mirror and place maintained patches on `omarchy-rs`.
- Add a `models-dev-pricing-only` ccusage-core feature that requires no downloaded LiteLLM snapshot.
- Consume Codex adapters by exact Git revision; do not use a branch dependency or submodule.
- Keep the ccusage API behind an internal omarchy-rs backend module.
- Preserve the upstream base revision and fork patch revision as machine-readable evidence.

## Boundaries

### Allowed Changes
- Cargo.toml
- Cargo.lock
- crates/dependency-probe/**
- crates/omarchy-agents/**
- docs/dependencies/**
- knowledge/decisions/adr-002-adopt-dependencies-through-a-security-and-compatibility-gate.md
- specs/task-ccusage-fork-integration.spec.md
- specs/task-agent-usage-parity.spec.md
- ../ccusage/rust/crates/ccusage-core/**
- ../ccusage/docs/omarchy-rs-fork.md

### Forbidden
- Do not put Omarchy state schemas, cache paths, app-server calls, activation, or rollback code in the ccusage fork.
- Do not enable build-time pricing downloads, provider network clients, or credential readers.
- Do not depend on a moving Git branch.
- Do not modify the fork's mirrored `main` branch with project-specific commits.

## Completion Criteria

### Rule: reproducible-source — Pin fork provenance

Scenario: Fork admission record pins both revisions
  Test: ccusage_fork_record_pins_upstream_and_patch_revisions
  Given the selected ccusage upstream base and fork patch commit
  When the fork admission record is validated
  Then both revisions are complete commit hashes and the Cargo dependency uses the patch revision

Scenario: Moving branch dependency is rejected
  Test: ccusage_fork_record_rejects_moving_dependency
  Given an admission record whose dependency has no exact revision
  When the fork admission record is validated
  Then validation returns a nonzero result

### Rule: offline-build — Keep the parser dependency offline

Scenario: Models-dev-only feature builds without LiteLLM input
  Test:
    Package: dependency-probe
    Filter: ccusage_models_dev_only_builds_offline
    Level: integration
    Test Double: locked_git_graph_and_synthetic_environment
  Given the fork is checked out at the admitted patch revision with an empty isolated Cargo network configuration
  When the Codex adapter is compiled with `models-dev-pricing-only` and Cargo offline mode
  Then compilation succeeds without `CCUSAGE_PRICING_JSON_PATH`

Scenario: Default missing pricing input still fails closed
  Test:
    Package: dependency-probe
    Filter: ccusage_default_missing_pricing_fails_closed
    Level: integration
    Test Double: recorded_fresh_target_build
  Given ccusage-core without `models-dev-pricing-only`, pricing input, or download feature
  When its build script selects the LiteLLM pricing source
  Then it returns an explicit missing-snapshot failure rather than contacting the network

### Rule: adapter-boundary — Reuse parsing without importing Omarchy behavior

Scenario: Codex fixture is parsed through the pinned adapter
  Test:
    Package: omarchy-agents
    Filter: pinned_ccusage_adapter_parses_synthetic_codex_fixture
    Level: integration
    Test Double: synthetic_agent_home
  Given the versioned synthetic Codex home
  When the internal dependency probe calls the pinned ccusage Codex adapter
  Then token events are returned without reading real HOME, credentials, or provider endpoints

Scenario: Fork contains no Omarchy-specific integration
  Test:
    Package: dependency-probe
    Filter: ccusage_fork_excludes_omarchy_specific_behavior
    Level: integration
    Test Double: pinned_upstream_diff_inventory
  Given the fork patch relative to its recorded upstream base
  When changed production paths and source terms are inspected
  Then no Omarchy state, cache, app-server, overlay, activation, or rollback implementation is present

## Out of Scope

- Completing the Omarchy Codex state-file parity collector.
- Claude and Fireworks production integration.
- Publishing ccusage crates to crates.io.
- Automatically pushing or opening an upstream pull request.
