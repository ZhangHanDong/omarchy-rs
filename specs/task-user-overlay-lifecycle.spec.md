spec: task
name: "User Overlay Deployment Lifecycle"
inherits: project
tags: [deployment, compatibility, rollback]
satisfies: [REQ-001]
depends: [task-octoscode-usage-parity]
estimate: 2d
---

<!-- lint-ack: verification-metadata-suggestion — Filesystem lifecycle scenarios use isolated TempDir roots and never exercise host package paths. -->

## Intent

Turn the Agent Usage pilot into a reversible user-level deployment controlled by
one `omarchy-rs` CLI. Installation and activation must remain independent from
official Omarchy package files, expose machine-readable status, and restore
upstream command resolution without network access.

## Decisions

- Install release executables under `$XDG_DATA_HOME/omarchy-rs/libexec`, defaulting to `~/.local/share/omarchy-rs/libexec`.
- Activate Agent Usage with one owned updater symlink under `$XDG_DATA_HOME/omarchy-rs/bin`; never overwrite a foreign file or symlink.
- Persist enabled providers in `$XDG_CONFIG_HOME/omarchy-rs/activation.json` and let the Rust updater pass provider-specific canary mode to its sibling shadows.
- Support `doctor`, `install`, `activate agent-usage`, `status --json`, and `rollback agent-usage` without sudo or network access.
- Install `omrs` as an owned short alias to the same `omarchy-rs` CLI while preserving the long command for compatibility.
- Report upstream fingerprint drift as unverified while retaining the Python fallback.

## Boundaries

### Allowed Changes
- Cargo.*
- crates/omarchy-cli/**
- crates/omarchy-compat/**
- docs/architecture.md
- docs/components/agent-usage.md
- docs/deployment.md
- README.md
- specs/task-user-overlay-lifecycle.spec.md

### Forbidden
- Do not write under `/usr/bin`, `/usr/share/omarchy`, or `/etc`.
- Do not overwrite an unowned file in `~/.local/bin`.
- Do not require sudo, pacman, network access, or an Omarchy source checkout.
- Do not remove the installed upstream Python collectors.

## Completion Criteria

### Rule: user-install — Install only user-owned artifacts

Scenario: Release siblings install atomically
  Test:
    Package: omarchy-rs
    Filter: install_copies_release_siblings_and_manifest
  Given isolated data and config homes plus a complete release sibling directory
  When `install` copies the Agent Usage executables
  Then every installed artifact and manifest is under the isolated user homes

Scenario: Install publishes one owned short CLI alias
  Test:
    Package: omarchy-rs
    Filter: install_creates_owned_omrs_alias
  Given an isolated user bin directory without an `omrs` entry
  When `install` publishes the CLI shims
  Then `omrs` and `omarchy-rs` resolve to the same installed executable

Scenario: Incomplete release refuses installation
  Test:
    Package: omarchy-rs
    Filter: install_rejects_missing_release_sibling
  Given a release sibling directory missing one required executable
  When `install` validates its source set
  Then it returns an error and publishes no installation manifest

### Rule: safe-activation — Activate only with verified precedence

Scenario: Agent Usage activation owns one shim
  Level: integration
  Test:
    Package: omarchy-rs
    Filter: activate_agent_usage_creates_owned_shim_and_config
  Given an installed manifest and a PATH where the overlay bin directory precedes upstream
  When `activate agent-usage` runs
  Then the updater shim targets the installed Rust updater and all three providers are enabled

Scenario: Foreign shim blocks activation
  Test:
    Package: omarchy-rs
    Filter: activate_refuses_foreign_shim
  Given an existing updater shim not recorded in the installation manifest
  When `activate agent-usage` runs
  Then it returns an error and changes neither the foreign shim nor activation config

Scenario: Unsupported precedence blocks activation
  Level: integration
  Test:
    Package: omarchy-rs
    Filter: activate_refuses_unsupported_precedence
  Given the overlay bin directory does not precede the upstream updater in PATH
  When `activate agent-usage` runs
  Then it returns an error and creates no updater shim

### Rule: observable-rollback — Report and reverse activation

Scenario: JSON status reports provider eligibility and drift
  Test:
    Package: omarchy-rs
    Filter: status_json_reports_backend_and_drift
  Given one verified provider fingerprint and one changed provider fingerprint
  When `status --json` inspects the installed overlay
  Then it reports activation, command resolution, and verified or unverified state without changing files

Scenario: Offline rollback removes only the owned shim
  Level: integration
  Test:
    Package: omarchy-rs
    Filter: rollback_restores_upstream_resolution_offline
  Given an activated owned updater shim and no network access
  When `rollback agent-usage` runs
  Then it removes the owned shim and activation config while leaving installed and upstream executables unchanged

## Out of Scope

- System-wide installation or package creation.
- Automatically restarting Hyprland or Quickshell.
- Replacing mutating, privileged, update, login, lock, or shutdown commands.
- Generating raw benchmark sample artifacts in this task.
