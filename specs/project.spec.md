spec: project
name: "omarchy-rs Project Contract"
---

## Intent

Build a reversible Rust acceleration layer for selected Omarchy user-space
tools. Preserve upstream behavior and upgrade independence while accepting only
replacements with measured operational benefit.

## Constraints

- Official Omarchy package-owned paths remain unmodified.
- Every replacement has fixture-driven parity tests and an offline rollback.
- Performance claims include reproducible baseline and candidate measurements.
- Production Rust forbids `unsafe` unless a task contract explicitly permits it.
- Logs, fixtures, and benchmark reports contain no credentials or prompt content.

## Decisions

- Rust workspace with one multicall CLI and domain crates added only as needed.
- Compatibility shims are selected explicitly; replacement is never all-or-nothing.
- Read-only, frequently invoked commands precede mutating or privileged commands.
- The installed upstream executable remains the fallback implementation.

## Boundaries

### Allowed Changes
- Cargo.toml
- rust-toolchain.toml
- crates/**
- tests/**
- benches/**
- packaging/**
- docs/**
- knowledge/**
- specs/**
- README.md
- CONTRIBUTING.md
- AGENTS.md

### Forbidden
- Do not edit `../omarchy/**` as part of omarchy-rs implementation tasks.
- Do not install files over `/usr/bin/omarchy*` or `/usr/share/omarchy/**`.
- Do not intercept `omarchy update`, pacman, sudo, login, lock, or shutdown paths in the initial phase.

## Completion Criteria

### Rule: package-isolation — Keep official Omarchy independently upgradeable

Scenario: Package layout excludes official paths
  Test:
    Package: omarchy-compat
    Filter: package_manifest_excludes_official_paths
  Given a generated omarchy-rs package manifest
  When its installation targets are inspected
  Then no target is under "/usr/bin", "/usr/share/omarchy", or "/etc/omarchy.conf"

Scenario: Unsupported precedence refuses activation
  Test:
    Package: omarchy-compat
    Filter: unsupported_precedence_refuses_activation
  Given the candidate shim directory does not precede the upstream command
  When replacement activation is requested
  Then activation returns a nonzero status and changes no shim

### Rule: evidence-gate — Require compatibility and measured value

Scenario: Replacement admission requires parity evidence
  Test:
    Package: omarchy-compat
    Filter: admission_requires_passing_parity_evidence
  Given a replacement has no passing fixture-driven parity result
  When admission is evaluated
  Then the replacement remains disabled by default

Scenario: Performance claim requires comparable measurements
  Test:
    Package: omarchy-compat
    Filter: admission_requires_comparable_benchmark
  Given a performance report omits either baseline or candidate measurements
  When admission is evaluated
  Then the replacement remains disabled by default

Scenario: Sensitive report is rejected
  Test:
    Package: omarchy-compat
    Filter: admission_rejects_sensitive_report
  Given a fixture or benchmark report contains a synthetic credential or prompt marker
  When evidence is validated
  Then validation returns a nonzero status and persists no report

Scenario: Unauthorized unsafe Rust is rejected
  Test:
    Package: omarchy-compat
    Filter: production_sources_forbid_unsafe
  Given production Rust source contains an unsafe block not permitted by a task Contract
  When project policy is checked
  Then the check returns a nonzero status

## Out of Scope

- Replacing the Omarchy distribution, Hyprland, Quickshell, systemd, or pacman.
- Kernel modules or Rust for Linux integration.
- Rust ports without a measured hot-path justification.
