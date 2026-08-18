# Dependency policy

`omarchy-rs` prefers a maintained open-source library over a private rewrite
when the library preserves the required Omarchy behavior and passes the gates
below. A smaller source tree is not valuable if it imports hidden network,
credential, licensing, or maintenance risk.

## Admission record

Every direct production dependency has a versioned record containing:

- package name, exact evaluated version, registry, and source repository;
- license expression and compatibility with MIT distribution;
- release recency, maintenance signals, and supported Rust version;
- default and enabled feature sets;
- normal, build, native, and transitive dependencies;
- `unsafe` usage and any build script;
- filesystem, process, credential, telemetry, and network access;
- relevant RustSec advisories and `cargo deny` findings;
- the exact upstream Omarchy behavior the dependency covers and does not cover;
- accept, adapt, isolate, or reject outcome with rationale.

The record is evidence for an ADR. It is not a substitute for fixture-driven
compatibility tests.

## Required checks

Candidate evaluation uses pinned versions and records the output of:

```bash
cargo metadata --locked --format-version 1
cargo tree -e features
cargo tree --duplicates
cargo audit
cargo deny check
```

Source review covers build scripts, enabled features, `unsafe`, filesystem and
credential readers, network clients, telemetry, subprocess creation, and cache
behavior. A clean advisory scan alone does not prove a dependency safe.

## Runtime boundaries

- Offline local-log parsing is preferred for the first Agent Usage pilot.
- Default builds disable unrelated CLI, TUI, GUI, image, telemetry, and network
  features when the candidate exposes feature flags for them.
- A dependency that reads OAuth credentials or calls a provider endpoint needs
  a separate requirement and explicit opt-in; it cannot enter through a local
  parser evaluation.
- Tests use synthetic homes and local fixtures. They never inspect a real
  `~/.codex`, `~/.claude`, token store, keychain, or environment secret.

## Upgrade policy

Direct dependency updates are reviewed as compatibility changes. The new
version repeats advisory, license, feature, and fixture checks before merge.
Version ranges in manifests may follow Cargo conventions, but release artifacts
are built from a committed `Cargo.lock`.
