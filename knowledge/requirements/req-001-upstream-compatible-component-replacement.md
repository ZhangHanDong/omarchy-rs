---
kind: requirement
id: REQ-001
title: "Upstream-compatible component replacement"
status: accepted
liveness: auto
tags: []
---

## Problem

Users need selected Omarchy tools to gain a Rust implementation without losing
the behavior expected by the Omarchy shell, scripts, or interactive CLI and
without official upgrades overwriting either implementation.

## Requirements

[REQ-001-COMPAT] Every activated replacement MUST preserve its declared argv, stdout, stderr, exit-status, environment, and file-format compatibility surface.

[REQ-001-UPGRADE] Installation MUST NOT overwrite pacman-owned Omarchy files or prevent `omarchy update` from updating official packages.

[REQ-001-ROLLBACK] Every activated replacement MUST have an offline operation that restores resolution to the installed upstream command.

[REQ-001-DRIFT] When a compatible upstream command changes, the system MUST report compatibility as unverified until differential tests pass for the new baseline; it MUST NOT block the upstream upgrade.

## Scenarios

Rule: REQ-001-COMPAT
Scenario: Compatible replacement
  Given a supported upstream command and deterministic fixtures
  When the upstream and Rust implementations receive identical inputs
  Then normalized stdout, stderr, exit status, JSON files, and permissions equal the declared baseline values

Rule: REQ-001-UPGRADE
Scenario: Official upgrade remains independent
  Given active omarchy-rs shims and installed official Omarchy packages
  When an official package upgrade replaces its owned files
  Then the upgrade exits with its upstream status and changes no omarchy-rs-owned file

Rule: REQ-001-ROLLBACK
Scenario: Offline rollback
  Given an activated replacement and no network access
  When the user disables that replacement
  Then `command -v` returns the installed absolute upstream executable path

Rule: REQ-001-DRIFT
Scenario: Upstream drift
  Given the upstream executable differs from the last verified baseline
  When compatibility status is inspected
  Then the replacement is reported as unverified without modifying upstream files

## Dependencies

None.

## Source Trace

- User direction on 2026-08-18: build a seamless user-space Rust upgrade that
  does not interfere with Omarchy system upgrades.
- Local Omarchy sources: `../omarchy/bin/omarchy`,
  `../omarchy/default/bash/env-bootstrap`, and `../omarchy/docs/file-layout.md`.

## Open Questions

None.

## Next

Implement through small parity contracts, beginning with the Agent Usage
collector pilot.
