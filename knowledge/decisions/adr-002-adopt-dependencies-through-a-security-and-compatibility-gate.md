---
kind: decision
id: ADR-002
title: "Adopt dependencies through a security and compatibility gate"
status: Accepted
liveness: auto
tags: [dependencies, security, compatibility]
---

## Context

Existing Rust and polyglot projects already parse Codex and Claude Code usage.
Reimplementing those formats would duplicate maintenance and edge-case work,
but adopting a package based only on feature claims can introduce credentials,
network access, build scripts, unsafe code, incompatible licensing, or an
unstable internal API.

## Decision

Prefer an existing open-source library when a pinned release passes the project
dependency policy, covers the required Omarchy behavior on synthetic fixtures,
and has an acceptable maintenance path. Keep an Omarchy-specific compatibility
adapter between third-party models and public state files.

Evaluate `tokenusage` without default features first for local Codex and Claude
logs. Evaluate ccusage's Rust adapter crates as a second candidate. Treat
`claude-usage` and any other credential-reading or network quota client as a
separate opt-in capability, not as part of the local parser pilot.

No candidate becomes a production dependency until ADR-002 is supplemented
with a pinned-version evaluation outcome and the dependency Contract passes.

### Agent Usage evaluation outcome (2026-08-18)

Against Omarchy `f32ebbdb730c4e8fe11e4046cef4267e466264ea`, no candidate
is accepted directly into a production crate:

- The ZhangHanDong ccusage fork is selected as the offline parsing basis at
  `03d8f07b867521cb74dd48af0379b3ffdc413c94`, based on upstream
  `95d0528c61c6748463f0fbaf119b6c2521a42b32`. Its maintained patch stack adds
  deterministic `models-dev-pricing-only` builds and a generic option for
  consumers that must preserve per-file events instead of deduplicating them.
  Exact Git pins and an
  internal backend contain the unpublished API risk; Omarchy-specific behavior
  is forbidden from the fork.
- `tokenusage` 1.5.2 is an adaptation fallback, not a direct dependency. Its
  default features can be disabled, but `reqwest` and provider
  credential/network implementations remain unconditional, and its MSRV 1.87
  exceeds this project's declared 1.85. It requires an upstream feature split
  or maintained fork before reconsideration.
- `claude-usage` 0.2.3 is isolated to a possible future opt-in online quota
  capability. It reads OAuth credentials and contacts Anthropic; it does not
  implement local transcript parsing.

The detailed evidence, feature graph results, and behavior gaps live in
`docs/dependencies/agent-usage.md`. This outcome authorizes the pinned fork in
the dependency probe and the subsequent Codex parity implementation; default
activation still requires the parity and performance Contracts.

## Consequences

Good, because the project reuses maintained parsing work while retaining a
small compatibility boundary and explicit security evidence.

Bad, because upstream library changes become part of the project's maintenance
surface and may require pinning, patching, or replacement.

## Alternatives Considered

- Reimplement every parser: rejected because it duplicates mature open-source
  work before compatibility or performance requires it.
- Execute a third-party CLI as a subprocess: retained only as a benchmark
  baseline because it adds startup and output-contract coupling.
- Adopt the first library that compiles: rejected because compilation does not
  establish security, licensing, compatibility, or maintenance quality.

## Next

Govern this decision through REQ-003 and the Agent Usage dependency evaluation
Contract.
