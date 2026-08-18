---
kind: requirement
id: REQ-003
title: "Safe and reliable third-party dependency admission"
status: accepted
liveness: auto
tags: []
---

## Problem

Reusing open-source libraries can reduce implementation and maintenance work,
but a dependency can silently expand runtime privileges, credential access,
network behavior, binary size, licensing duties, or compatibility risk.

## Requirements

[REQ-003-INVENTORY] Every proposed direct production dependency MUST have a pinned-version admission record containing source, license, maintenance, MSRV, feature, transitive dependency, build-script, unsafe, credential, network, telemetry, advisory, and behavior-coverage findings.

[REQ-003-LICENSE] Every distributed dependency MUST have a license compatible with the project MIT License and its attribution obligations MUST be recorded.

[REQ-003-MINIMAL] Production manifests MUST disable unrelated default features when a smaller supported library surface provides the required behavior.

[REQ-003-SECRETS] Tests and benchmarks MUST NOT read real credential stores, provider tokens, agent homes, prompt contents, or environment secrets.

[REQ-003-NETWORK] A dependency that reads credentials or contacts a provider endpoint MUST remain excluded from default local parsing until a separate accepted requirement authorizes that boundary.

[REQ-003-ADVISORY] A dependency with an unmitigated applicable security advisory or denied license MUST NOT be admitted.

## Scenarios

Rule: REQ-003-INVENTORY
Scenario: Complete candidate record
  Given a pinned candidate crate and synthetic behavior fixtures
  When dependency evaluation completes
  Then every required admission field contains evidence and an accept, adapt, isolate, or reject outcome

Rule: REQ-003-LICENSE
Scenario: Compatible license
  Given a candidate dependency graph and collected license expressions
  When license policy is evaluated
  Then every distributed package is allowed by the MIT-compatible policy and required notices are listed

Rule: REQ-003-MINIMAL
Scenario: Minimal feature surface
  Given a candidate whose default features include unrelated interfaces
  When the production feature set is generated
  Then unrelated supported CLI, TUI, GUI, image, telemetry, and network features are disabled

Rule: REQ-003-SECRETS
Scenario: Isolated evaluation
  Given synthetic homes containing canary values and real HOME containing a different canary
  When compatibility tests and benchmarks execute
  Then persisted artifacts contain the synthetic identifier and zero occurrences of the real-HOME canary

Rule: REQ-003-NETWORK
Scenario: Credential or network boundary
  Given a candidate opens a credential store or provider endpoint
  When default local-parser admission is evaluated
  Then the outcome is isolate or reject and no production default feature enables that behavior

Rule: REQ-003-ADVISORY
Scenario: Denied advisory or license
  Given an applicable unmitigated advisory or denied license in the resolved graph
  When dependency admission is evaluated
  Then the result is reject and no production manifest references the candidate

## Dependencies

- REQ-001

## Source Trace

- User direction on 2026-08-18: reuse safe and reliable open-source libraries instead of reimplementing Codex and Claude Code usage logic.
- `docs/dependency-policy.md`

## Open Questions

None.

## Next

Implement through the Agent Usage dependency evaluation Contract before the
collector parity pilot.
