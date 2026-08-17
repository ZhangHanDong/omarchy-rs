---
kind: decision
id: ADR-001
title: "Install as a reversible overlay"
status: Accepted
liveness: auto
tags: [architecture, compatibility, packaging]
---

## Context

Official Omarchy packages own `/usr/bin/omarchy-*` and
`/usr/share/omarchy/**`. Writing replacements into those locations would cause
file conflicts or allow a normal system upgrade to overwrite `omarchy-rs`.
Using `omarchy dev link` would redirect the whole runtime rather than replace a
small set of user-space tools.

## Decision

Install `omarchy-rs` outside official Omarchy package paths. The canonical
multicall executable lives under `/usr/local/lib/omarchy-rs/`, and activation
creates selected shims under `/usr/local/bin/`. A shim delegates to an absolute
upstream executable when disabled, unsupported, or explicitly asked to fall
back. Installation must probe command precedence before activation.

Never overwrite files owned by the `omarchy` or `omarchy-settings` packages,
never change `/etc/omarchy.conf`, and never intercept system update commands in
the initial scope.

## Consequences

Good, because official packages remain intact and upgrades remain independent.
Each shim can be removed to restore upstream behavior immediately.

Bad, because absolute calls into `$OMARCHY_PATH/bin` bypass the overlay, and a
top-level `omarchy` proxy requires a separate, stricter compatibility contract.

## Alternatives Considered

- Install replacement files into `/usr/bin`: rejected because pacman owns it.
- Maintain a complete Omarchy fork: rejected because it couples unrelated
  system changes to this user-space optimization project.
- Permanently enable `omarchy dev link`: rejected because it redirects the
  complete Omarchy runtime and changes upgrade behavior.

## Next

Govern this decision through REQ-001 and its satisfying task contracts.
