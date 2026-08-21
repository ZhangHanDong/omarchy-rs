spec: task
name: "Unified Plugin Manager"
inherits: project
tags: [plugins, cli, ownership, health]
satisfies: [REQ-009]
estimate: 1d
---

<!-- lint-ack: verification-metadata-suggestion — Process behavior uses isolated executable stubs and synthetic configuration roots. -->

## Intent

Add one `omarchy-rs plugin` control plane for the Cleaner, Skills, and Network
Inspector plugins. Make stale QML, missing dependencies, enablement, safe
updates, and restart guidance observable without modifying Omarchy itself.

## Decisions

- Accept only `cleaner`, `skills`, and `network-inspector` as component identifiers and expose their canonical Omarchy plugin ids.
- Embed every managed plugin file in the Rust binary and compare SHA-256 hashes against a versioned ownership receipt.
- Use direct argv to call only the public `omarchy plugin list|enable|disable` interface; never invoke a shell.
- Let `update` without a component refresh installed owned plugins only; do not install absent plugins implicitly.
- Return JSON from every inventory and mutation command with `restartRecommended` and `restarted`; invoke `omarchy-restart-shell` directly only for a successful `update --restart` request.
- Require Sniffnet only for Network Inspector and report the missing executable as a typed dependency problem.

## Boundaries

### Allowed Changes
- crates/omarchy-plugins/**
- crates/omarchy-cli/src/main.rs
- src/lib.rs
- README.md
- docs/components/plugin-manager.md
- knowledge/requirements/req-009-unified-plugin-control-plane.md
- specs/task-unified-plugin-manager.spec.md

### Forbidden
- Do not modify sibling Omarchy, `/usr/share/omarchy`, shell configuration directly, system packages, or plugin files not owned by omarchy-rs.
- Do not invoke a shell, sudo, pkexec, package manager, or shell restart without an explicit `update --restart` flag.

## Completion Criteria

### Rule: normalized-inventory — Report one stable plugin model

Scenario: Inventory includes all managed plugins
  Test:
    Package: omarchy-rs
    Filter: plugins_inventory_normalizes_owned_components
  Given isolated plugin roots and a synthetic Omarchy list response
  When `plugin list --json` runs
  Then three canonical records include install, ownership, current, dependency, enabled, and version fields

Scenario: Doctor exposes stale and missing dependency states
  Test:
    Package: omarchy-rs
    Filter: plugins_doctor_reports_actionable_drift
  Given stale owned bytes and missing Sniffnet in an isolated PATH
  When `plugin doctor --json` runs
  Then typed stale-plugin and missing-dependency problems identify their components

### Rule: guarded-plugin-mutations — Change only exact owned destinations

Scenario: Install writes embedded files and restart guidance
  Test:
    Package: omarchy-rs
    Filter: plugins_install_is_owned_and_requests_restart
  Given an absent synthetic plugin destination
  When one known plugin is installed
  Then embedded files and their receipt are written and restartRecommended is true

Scenario: Foreign or modified destinations are refused
  Test:
    Package: omarchy-rs
    Filter: plugins_mutations_refuse_foreign_or_modified_files
  Given foreign and modified synthetic plugin destinations
  When install, update, or uninstall is attempted
  Then every operation fails without changing destination bytes

Scenario: Bulk update preserves absent plugins
  Test:
    Package: omarchy-rs
    Filter: plugins_bulk_update_refreshes_installed_only
  Given one valid installed plugin and two absent plugins
  When update runs without a component
  Then only the installed plugin receives current embedded bytes

Scenario: Explicit restart runs only after a successful update
  Test:
    Package: omarchy-rs
    Filter: plugins_update_restart_is_explicit_and_ordered
  Given isolated plugin and restart executables
  When update runs without and then with `--restart`
  Then only the explicit request invokes the direct restart executable after plugin bytes are current

Scenario: Enable and uninstall use direct Omarchy argv
  Test:
    Package: omarchy-rs
    Filter: plugins_enable_and_uninstall_use_direct_omarchy_argv
  Given an isolated Omarchy executable and one owned plugin
  When enable followed by uninstall runs
  Then exact enable and disable argv are recorded without a shell before owned files are removed

Scenario: Invalid components and unavailable Omarchy fail closed
  Test:
    Package: omarchy-rs
    Filter: plugins_reject_unknown_component_or_missing_omarchy
  Given an unknown component or unavailable Omarchy executable
  When mutation is requested
  Then a typed error is returned and no plugin destination changes

## Out of Scope

- Managing third-party or first-party Omarchy plugins.
- Treating Learn or Agent Usage overlays as QML plugins.
- A graphical plugin-manager panel.
- Downloading plugin code or packages.
