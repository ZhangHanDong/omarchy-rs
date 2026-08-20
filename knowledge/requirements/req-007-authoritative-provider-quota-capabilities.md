---
kind: requirement
id: REQ-007
title: "Authoritative provider quota capabilities"
status: accepted
liveness: auto
tags: [agent-usage, quota, grok, octoscode]
---

## Problem

The Agent Usage panel currently leaves Grok and Octoscode quota windows blank.
For a personal Grok account there is no supported quota API, while Octoscode
can route requests to providers with different and sometimes unavailable quota
surfaces. Blank output is ambiguous, but inferred windows would be misleading.

## Requirements

[REQ-007-AUTHORITY] The system MUST display only quota windows returned by an authoritative account or provider interface and MUST NOT infer Session, 5-hour, Weekly, or Monthly percentages from local token totals.

[REQ-007-GROK] A personal Grok record MUST identify the single shared Weekly pool as unavailable through the CLI and direct the user to Grok Settings → Usage without requesting a Business Management Key or reading browser state.

[REQ-007-OCTOS] An Octoscode record MUST identify provider/model quotas as unavailable when its public ledger supplies token usage but no authoritative quota snapshot.

[REQ-007-PRIVACY] Quota capability reporting MUST NOT read, persist, or emit Management Keys, API keys, browser cookies, prompts, responses, or credentials.

## Scenarios

Rule: REQ-007-AUTHORITY
Scenario: Local totals never become quota windows
  Given synthetic local usage with nonzero token totals and no quota snapshot
  When either Rust collector creates its panel record
  Then the limits array contains zero inferred percentages

Rule: REQ-007-GROK
Scenario: Personal Grok reports the supported route
  Given a synthetic Grok home with local completed turns
  When the Rust collector creates its panel record
  Then limits remain empty and the record directs the user to Settings → Usage

Rule: REQ-007-OCTOS
Scenario: Octoscode does not infer provider quotas
  Given a synthetic Octoscode ledger containing models and token totals
  When the Rust collector creates its panel record
  Then limits remain empty and the record states that provider quotas are unavailable

Rule: REQ-007-PRIVACY
Scenario: Capability status requires no sensitive account material
  Given synthetic local usage fixtures and no credential files
  When both collectors create records
  Then neither collector accesses or emits account secrets

## Dependencies

- REQ-001

## Source Trace

- User-approved design on 2026-08-21: for a personal Grok account, do not use
  the Business Management API; show the official Settings → Usage route and do
  not fabricate unavailable windows.
- xAI Grok FAQ, checked 2026-08-21: paid Grok uses one shared Weekly pool whose
  percentage and reset are shown in Settings → Usage.
- Local Octoscode protocol inspection on 2026-08-21: ledgers expose token and
  model usage but no authoritative provider quota snapshot.

## Open Questions

None.

## Next

Implement capability-aware status in the Grok and Octoscode Rust collectors
and document how users obtain the authoritative values.
