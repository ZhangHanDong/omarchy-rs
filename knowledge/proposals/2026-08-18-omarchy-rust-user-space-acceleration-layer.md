---
kind: proposal
id: LEP-001
title: "Omarchy Rust user-space acceleration layer"
status: proposed
liveness: n/a
tags: []
---

# Omarchy Rust user-space acceleration layer

## Context

Omarchy exposes hundreds of user-space commands, mostly as Bash scripts. Most
are small orchestration wrappers and do not benefit from a rewrite, while a
smaller set repeatedly scans files, parses JSON, or starts several helper
processes from the long-running Quickshell desktop. Those hot paths can consume
unnecessary CPU time, I/O, and energy and have weak type boundaries.

## Motivation

Improve selected Omarchy user-space tools without taking ownership of the
operating system, replacing the desktop shell, or blocking official upgrades.
Selection must be driven by measurements and operational risk rather than by a
goal of maximizing the amount of Rust.

## Goals

- Preserve the observable behavior of each replaced upstream command.
- Reduce measured process creation, CPU time, I/O, or memory on proven hot paths.
- Keep official Omarchy packages installed and upgradeable.
- Make every replacement independently disableable and removable.

## Non-Goals

- Reimplementing Omarchy as a distribution.
- Replacing Hyprland, Quickshell, systemd, pacman, or other system services.
- Rewriting short, infrequently executed Bash wrappers without measured benefit.
- Kernel development or a dependency on Rust for Linux.

## Decision

Create `omarchy-rs` as an independent Rust workspace and package. It provides a
multicall binary plus narrowly scoped compatibility shims for selected Omarchy
commands. The official implementation remains installed as the compatibility
baseline and fallback. Initial work targets read-only Agent Usage collectors;
additional components require their own benchmark and parity contract.

## Compatibility

- CLI and public API: preserve argv, stdout, stderr, exit status, and documented
  environment-variable behavior for every activated replacement.
- File formats: preserve upstream JSON schemas and use atomic replacement for
  generated state files.
- Existing specs and KLL artifacts: each implementation task satisfies the
  relevant `REQ-*` documents and binds scenarios to executable tests.

## Migration Plan

Install the implementation outside pacman-owned Omarchy paths. Activation adds
only explicitly selected shims. Deactivation removes those shims and exposes
the still-installed upstream commands immediately. An upstream compatibility
check runs after upgrades without delaying or rejecting the upgrade itself.

## Security Considerations

Read-only replacements run with the invoking user's privileges and must not
expand the files, commands, or network endpoints read by upstream. Commands
with system mutation or privilege escalation are excluded from the first phase.

## Privacy Considerations

Agent logs and usage records remain local. Benchmarks store aggregate timing,
resource, and fixture metadata but no prompt contents, credentials, or telemetry.

## Risks and Assumptions

### Assumptions

- `/usr/local/bin` precedes `/usr/bin` in supported interactive and graphical
  sessions. Invalidated if an installation probe reports different precedence.
- Selected upstream commands have deterministic fixture-driven behavior.
  Invalidated if parity cannot be evaluated without external mutable state.

### Risks

- Upstream changes an input or output contract. Mitigation: compatibility
  fingerprints, differential tests, and a visible fallback status.
- A shim recursively invokes itself. Mitigation: fallback uses a resolved,
  absolute upstream executable path and recursion tests.
- A benchmark rewards synthetic behavior. Mitigation: publish fixture shape,
  cold/warm conditions, sample count, and both baseline and candidate results.

## Consequences

Good, because expensive user-space logic gains Rust's typed parsing, bounded
concurrency, and single-process execution without coupling system upgrades to
the experiment.

Bad, because an overlay adds command-precedence and upstream-compatibility
maintenance, and Rust does not improve commands dominated by external tools.

## Alternatives Considered

- Full Omarchy fork: rejected because it makes every upstream upgrade a merge
  and packaging responsibility.
- Replace all Bash commands: rejected because most commands are small wrappers
  whose cost is dominated by the programs they invoke.
- Modify `/usr/share/omarchy` or `/usr/bin`: rejected because pacman legitimately
  replaces those paths during upgrades.
- Add a daemon immediately: rejected until measurements show process startup or
  repeated scans remain material after the first replacements.

## Prior Art

- Omarchy's existing CLI metadata/router and shell test conventions are the
  compatibility baseline.
- Unix command interposition through PATH is the deployment precedent.

## Unresolved Questions

- What minimum improvement is required to activate a replacement by default?
- Should installation initially be a PKGBUILD, an installer, or both?

## Produces

- ADR-001
- REQ-001
- REQ-002
