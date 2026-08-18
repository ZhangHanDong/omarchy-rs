spec: task
name: "Agent Usage Dependency Evaluation"
inherits: project
tags: [dependencies, security, compatibility]
satisfies: [REQ-003]
estimate: 2d
---

## Intent

Evaluate maintained open-source libraries for the Agent Usage pilot before
writing a parser. Produce reproducible security, licensing, feature, maintenance,
and fixture-coverage evidence and record whether each candidate is accepted,
adapted, isolated, or rejected.

## Decisions

- Evaluate pinned releases of `tokenusage`, ccusage Rust adapters, and `claude-usage`.
- Test `tokenusage` with default features disabled before considering any larger feature set.
- Use synthetic Codex and Claude homes; evaluation performs no real credential access and no provider network call.
- Treat local-log parsing and online quota retrieval as separate security boundaries.
- Record facts in `docs/dependencies/agent-usage.md`; update ADR-002 with the selected architecture only after evidence is complete.

## Boundaries

### Allowed Changes
- Cargo.toml
- Cargo.lock
- deny.toml
- crates/dependency-probe/**
- tests/dependency_policy/**
- fixtures/agent_usage/**
- docs/dependencies/**
- docs/dependency-policy.md
- knowledge/decisions/adr-002-adopt-dependencies-through-a-security-and-compatibility-gate.md
- knowledge/requirements/req-003-safe-and-reliable-third-party-dependency-admission.md
- specs/task-agent-usage-dependency-evaluation.spec.md
- specs/task-agent-usage-parity.spec.md

### Forbidden
- Do not add a candidate to a production crate during this evaluation.
- Do not access real `~/.codex`, `~/.claude`, keychains, credential files, tokens, prompts, or provider endpoints.
- Do not accept a candidate from popularity, download count, or benchmark speed alone.
- Do not weaken Omarchy output compatibility to fit a candidate library model.

## Completion Criteria

### Rule: evidence-completeness — Record dependency evidence

Scenario: Candidate inventory is complete
  Test: dependency_records_include_required_fields
  Given pinned candidate versions for tokenusage, ccusage Rust adapters, and claude-usage
  When the admission report is validated
  Then each candidate contains every field required by the dependency policy and cites its source evidence

Scenario: Missing evidence blocks a decision
  Test: incomplete_dependency_record_is_rejected
  Given a candidate record missing license, unsafe, credential, network, advisory, or behavior-coverage evidence
  When the admission report is validated
  Then validation returns a nonzero status and ADR-002 contains no accept outcome for that candidate

### Rule: boundary-safety — Keep evaluation isolated

Scenario: Candidate probe uses synthetic homes only
  Test: dependency_probe_uses_only_synthetic_homes
  Given isolated Codex and Claude fixture homes and canary environment secrets
  When candidate probes execute with network access denied
  Then probes read only fixture paths and persisted artifacts contain zero secret canaries

Scenario: Credential-reading client remains isolated
  Test: credential_or_network_candidate_cannot_be_default
  Given source evidence that a candidate reads credentials or contacts a provider endpoint
  When default local-parser admission is evaluated
  Then the recorded outcome is isolate or reject

### Rule: behavior-fit — Measure library coverage of Omarchy behavior

Scenario: Local parser coverage matrix is generated
  Test: candidate_coverage_matrix_matches_fixtures
  Given versioned valid, empty, malformed, duplicate, cold, and warm fixtures
  When each local parser candidate is compared with the pinned Omarchy baseline
  Then the report lists every matched, adaptable, and missing output behavior without reading real user state

Scenario: Candidate model mismatch remains visible
  Test: missing_omarchy_behavior_prevents_direct_acceptance
  Given a candidate omits an Omarchy rate-limit, reset, account, diagnostic, or state-file behavior
  When admission is evaluated
  Then the outcome is adapt, isolate, or reject rather than direct accept

### Rule: policy-gate — Reject unsafe dependency graphs

Scenario: Denied advisory or license rejects candidate
  Test: denied_advisory_or_license_rejects_candidate
  Given a resolved candidate graph with an applicable unmitigated advisory or denied license
  When policy checks run
  Then the candidate outcome is reject and no production manifest references it

Scenario: Accepted feature set is minimal
  Test: accepted_candidate_disables_unrelated_features
  Given a candidate passes behavior, license, advisory, and maintenance checks
  When its proposed production feature set is inspected
  Then unrelated supported CLI, TUI, GUI, image, telemetry, and network features are disabled

## Out of Scope

- Implementing the production Agent Usage collector.
- Calling Anthropic, OpenAI, or Fireworks provider APIs.
- Selecting the final performance activation threshold.
- Evaluating unrelated Omarchy component libraries.
