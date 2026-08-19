spec: task
name: "Single Package Release"
inherits: project
tags: [release, crates-io, workspace]
satisfies: [REQ-001, REQ-003]
depends: [task-user-overlay-lifecycle, task-grok-usage-collector]
estimate: 1d
---

## Intent

Publish the complete user overlay as one installable `omarchy-rs` crate. Merge
the internal build boundary without changing the installed executable set,
runtime compatibility, rollback behavior, or the pinned provenance of adapted
ccusage parsing behavior.

## Decisions

- crates.io exposes exactly one package named `omarchy-rs` at version `0.1.1`.
- Agent collectors, compatibility routing, and lifecycle management compile as
  internal modules of that package while retaining their current binaries.
- Remove production Git dependencies from the published manifest; keep the
  ccusage fork revision in source and dependency documentation as provenance.
- [platform-specific] Validate the packaged archive and a clean `cargo install --path` before any
  immutable tag or registry upload.

## Boundaries

### Allowed Changes
- Cargo.*
- src/**
- crates/omarchy-agents/**
- crates/omarchy-compat/**
- crates/omarchy-cli/**
- crates/dependency-probe/**
- docs/**
- README.md
- specs/task-single-package-release.spec.md

### Forbidden
- Do not modify the sibling Omarchy or ccusage repositories.
- Do not publish internal support crate names.
- Do not change installed executable names, state paths, or upstream fallback paths.
- Do not create a Git tag or registry release before package verification passes.

## Completion Criteria

### Rule: single-public-package — One installable registry package

Scenario: Manifest exposes one public package
  Test: release_manifest_has_single_public_package
  Given the repository release manifest
  When its package and workspace membership are inspected
  Then `omarchy-rs` version `0.1.1` is the only publishable package

Scenario: Registry build has no Git dependency
  Test: release_manifest_has_no_git_dependencies
  Given the public package dependency graph
  When production dependency sources are inspected
  Then no dependency resolves from a Git URL

### Rule: release-surface — Packaging preserves the overlay

Scenario: Runtime executable set remains complete
  Test: release_binary_set_is_complete
  Given the single package binary targets and install manifest
  When their names are compared
  Then all six managed runtime executables are present exactly once

Scenario: Unknown release artifact is rejected
  Test: install_rejects_missing_release_sibling
  Given an incomplete release sibling directory
  When the lifecycle manager validates an installation
  Then installation fails without changing the user overlay

## Out of Scope

- Publishing ccusage fork crates.
- Adding non-Linux packaging artifacts.
- Changing provider aggregation semantics or performance claims.
