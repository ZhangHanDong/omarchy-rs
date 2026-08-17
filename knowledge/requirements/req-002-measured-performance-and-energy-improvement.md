---
kind: requirement
id: REQ-002
title: "Measured performance and energy improvement"
status: accepted
liveness: auto
tags: []
---

## Problem

Rewriting an infrequent shell wrapper in Rust can increase maintenance cost
without changing user-visible performance or energy use. Replacement decisions
therefore need reproducible evidence from realistic workloads.

## Requirements

[REQ-002-BASELINE] Every proposed replacement MUST publish a reproducible baseline and candidate benchmark using identical fixtures, environment, warm-up, and sample-count settings.

[REQ-002-METRICS] Benchmarks MUST report wall-clock time, CPU time, maximum RSS, bytes read and written when available, and child-process count.

[REQ-002-GATE] A replacement MUST NOT be enabled by default unless it improves at least one declared primary metric without exceeding the compatibility and resource-regression limits in its task contract.

[REQ-002-PRIVACY] Benchmark artifacts MUST NOT contain prompt contents, credentials, access tokens, or user telemetry.

## Scenarios

Rule: REQ-002-BASELINE
Scenario: Reproducible comparison
  Given a versioned fixture and benchmark configuration
  When the upstream and Rust implementations are measured
  Then the report contains both results and identical fixture, environment, warm-up, and sample-count metadata

Rule: REQ-002-METRICS
Scenario: Required metrics
  Given a completed upstream and Rust benchmark comparison
  When the report is decoded
  Then wall-clock time, CPU time, maximum RSS, available I/O bytes, and child-process count fields are present for both implementations

Rule: REQ-002-GATE
Scenario: No proven primary improvement
  Given a candidate that does not improve a declared primary metric
  When activation eligibility is evaluated
  Then the eligibility result contains `default_enabled=false`

Scenario: Resource regression
  Given a candidate that exceeds a task contract resource limit
  When activation eligibility is evaluated
  Then the eligibility result contains `default_enabled=false` and a resource-regression reason

Rule: REQ-002-PRIVACY
Scenario: Sensitive fixture data
  Given fixtures containing synthetic markers for secrets and prompt content
  When benchmark artifacts are generated
  Then a byte search finds zero occurrences of those markers in persisted reports

## Dependencies

- REQ-001

## Source Trace

- User direction on 2026-08-18: Rust migrations must produce real performance,
  energy, stability, or modernization value rather than maximize Rust coverage.

## Open Questions

None.

## Next

Define numeric thresholds in each component's parity task contract before
implementation.
