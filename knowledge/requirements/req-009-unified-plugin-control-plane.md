---
kind: requirement
id: REQ-009
title: "Unified omarchy-rs plugin control plane"
status: accepted
liveness: auto
tags: [plugins, cli, ownership, health]
---

## Problem

omarchy-rs plugins currently expose separate install commands. Users cannot
inspect version drift, dependencies, enablement, or required shell refreshes
from one stable interface, which makes an upgraded binary easy to pair with
stale QML files.

## Requirements

[REQ-009-INVENTORY] The CLI MUST inventory every omarchy-rs-owned QML plugin with installation, ownership, embedded-version, dependency, and Omarchy enablement state.

[REQ-009-MUTATION] Install, update, enable, and uninstall MUST use explicit plugin identifiers, direct argv, and existing ownership hashes; foreign or modified files MUST never be overwritten or removed.

[REQ-009-UPDATE] Updating without an identifier MUST update only already-installed owned plugins and MUST leave absent plugins absent.

[REQ-009-DOCTOR] Doctor MUST report actionable typed problems for missing dependencies, stale embedded files, corrupt ownership, and unavailable Omarchy plugin commands.

[REQ-009-RELOAD] Every successful file or enablement mutation MUST state whether an Omarchy shell restart is recommended; `plugin update --restart` MUST invoke the configured restart executable directly only after a successful update.

## Scenarios

Rule: REQ-009-INVENTORY
Scenario: Inventory normalizes all owned plugins
  Given isolated plugin roots and an Omarchy list response
  When plugin inventory runs
  Then Cleaner, Skills, and Network Inspector expose normalized health state

Rule: REQ-009-MUTATION
Scenario: Foreign files remain untouched
  Given a plugin directory without a matching ownership receipt
  When install, update, or uninstall runs
  Then the operation fails before any file changes

Rule: REQ-009-UPDATE
Scenario: Update changes installed plugins only
  Given one installed owned plugin and two absent plugins
  When update runs without an identifier
  Then only the installed plugin is refreshed from embedded bytes

Rule: REQ-009-DOCTOR
Scenario: Doctor reports actionable drift
  Given stale plugin bytes and a missing required executable
  When doctor runs
  Then typed stale and dependency diagnostics are returned

Rule: REQ-009-RELOAD
Scenario: Restart remains explicit
  Given successful plugin updates with and without the restart flag
  When their results are serialized
  Then the default recommends a restart while the explicit form invokes one direct restart executable

## Dependencies

- REQ-004
- REQ-008

## Source Trace

- User-approved roadmap on 2026-08-22: add one plugin manager before creating more plugins.
- Existing Cleaner, Skills, and Network Inspector ownership receipts and Omarchy public plugin CLI.

## Open Questions

None.
