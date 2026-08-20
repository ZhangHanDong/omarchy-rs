spec: task
name: "Rust Workspace Cleaner and Omarchy Plugin"
inherits: project
tags: [cleaner, rust, omarchy-plugin, performance]
satisfies: [REQ-002, REQ-004]
depends: [task-single-package-release]
estimate: 4d
---

<!-- lint-ack: platform-decision-tag — This Linux-only Omarchy component intentionally names Cargo, npm, and XDG paths. -->
<!-- lint-ack: verification-metadata-suggestion — Destructive-path tests use isolated TempDir roots and synthetic ownership metadata only. -->
<!-- lint-ack: testability — The flagged `cleaner` command name is a literal executable namespace; assertions name exact files and argv. -->
<!-- lint-ack: output-mode-coverage — The CLI has stdout JSON only and no file-output flag; persisted plans are state, not an output mode. -->
<!-- lint-ack: flag-combination-coverage — `--json` is the only output flag; candidate selectors and plan tokens do not alter output rendering. -->

## Intent

Add a Rust cleanup engine and an Omarchy bar panel that inventory regenerable
Rust and Node build artifacts, prioritizing `~/Work`. Keep scanning read-only
and require an exact, revalidated cleanup plan before deleting user-owned
artifacts, while measuring whether Rust materially reduces large-tree scan cost.

## Decisions

- Expose `omarchy-rs cleaner scan|plan|apply|install-plugin|uninstall-plugin`; `scan`, `plan`, and `apply` support JSON output consumed by QML.
- Default the scan root to `$HOME/Work`, while allowing one explicit absolute user-owned root per invocation.
- Recognize Rust `target` only beside `Cargo.toml` plus Cargo cache evidence; recognize Node `node_modules`, `.next/cache`, `.turbo`, and `.vite` only beneath a project with `package.json`.
- Exclude generic `build` and `dist`, `.git` contents, symlinks, different-device descendants, non-user-owned paths, and candidates written within the last five minutes.
- Persist versioned plans under `$XDG_STATE_HOME/omarchy-rs/cleaner/plans`; apply requires the plan id and its confirmation token and revalidates identity and evidence before removal.
- Embed the QML plugin in the single `omarchy-rs` crate and install it only under `$XDG_CONFIG_HOME/omarchy/plugins/omarchy-rs.cleaner`; never edit `/usr/share/omarchy`.
- Compare a release Rust scan with a deterministic Python reference over the same generated tree after three warmups and thirty measured runs. Primary metrics are median wall time and peak RSS; Rust must improve at least one by 40% without regressing the other by more than 10%.

## Boundaries

### Allowed Changes
- Cargo.*
- src/**
- crates/omarchy-cli/**
- crates/omarchy-cleaner/**
- plugins/omarchy-rs.cleaner/**
- benches/**
- docs/**
- knowledge/requirements/req-004-safe-user-space-build-artifact-cleanup.md
- specs/task-workspace-cleaner-plugin.spec.md
- README.md

### Forbidden
- Do not modify `../omarchy/**`, `/usr/share/omarchy/**`, `/usr/bin/**`, `/etc/**`, package databases, journals, snapshots, containers, or browser profiles.
- Do not invoke `sudo`, `pkexec`, pacman, or a shell command to scan or delete.
- Do not follow a symlink or accept a relative, root, home-directory, or outside-plan deletion target.
- Do not run cleanup against the real `~/Work` during tests or benchmarks.
- Do not claim energy improvement without direct energy measurements.

## Completion Criteria

### Rule: validated-inventory — Report only proven build artifacts

Scenario: Rust and Node artifacts are classified
  Test: cleaner_scan_classifies_validated_artifacts
  Given a synthetic Work tree with Cargo and Node project markers and build evidence
  When `cleaner scan --json` inventories the tree
  Then the report contains candidate kind, canonical path, project root, evidence, bytes, and file count for each validated artifact

Scenario: Ambiguous and overlapping directories are excluded
  Test: cleaner_scan_excludes_ambiguous_and_overlapping_paths
  Given generic `target`, `build`, and `dist` directories plus a nested `.vite` inside selected `node_modules`
  When the cleaner normalizes candidates
  Then generic directories are absent and total bytes count the nested cache once

### Rule: contained-scan — Stay inside the approved filesystem boundary

Scenario: Symlink and git descendants are not traversed
  Test: cleaner_scan_does_not_follow_symlinks_or_git
  Given a synthetic root with an outside-pointing symlink and build-shaped content under `.git`
  When the root is scanned
  Then neither outside content nor `.git` content appears in candidates or byte totals

Scenario: Unsafe scan roots are rejected
  Test: cleaner_rejects_unsafe_roots
  Given root, home, relative, missing, and non-directory scan paths
  When each path is passed as a cleaner root
  Then each invocation returns an error before traversing files

### Rule: planned-delete — Require exact intent before removal

Scenario: Scan and plan preserve all artifacts
  Test: cleaner_scan_and_plan_are_read_only
  Given validated artifacts and an isolated state directory
  When scan and plan complete
  Then all artifact contents and metadata remain present and one versioned plan is persisted under the isolated state directory

Scenario: Confirmed plan removes only selected candidates
  Test: cleaner_apply_removes_only_confirmed_candidates
  Given a plan selecting one of two validated artifacts
  When apply receives the matching plan id and confirmation token
  Then only the selected artifact is absent and JSON reports its reclaimed bytes

Scenario: Missing or incorrect confirmation is rejected
  Test: cleaner_apply_rejects_missing_or_wrong_confirmation
  Given a valid persisted cleanup plan
  When apply receives no token or a different token
  Then it returns an error and every candidate remains present

### Rule: revalidated-delete — Refuse stale or substituted candidates

Scenario: Candidate identity changed after planning
  Test: cleaner_apply_skips_replaced_candidate
  Given a planned artifact is replaced with another directory before apply
  When the confirmed plan is applied
  Then the replacement remains present and JSON reports an identity mismatch skip

Scenario: Candidate changed recently
  Test: cleaner_apply_skips_recent_candidate
  Given a planned artifact gains a file modified within five minutes
  When the confirmed plan is applied
  Then the artifact remains present and JSON reports a recent-write skip

### Rule: plugin-lifecycle — Keep the UI user-owned and reversible

Scenario: Plugin installation is isolated
  Test: cleaner_plugin_install_is_user_owned
  Given isolated XDG config and a crate-embedded plugin
  When `cleaner install-plugin` runs
  Then a valid manifest and QML panel exist only under `omarchy-rs.cleaner` in the isolated user plugin directory

Scenario: Foreign plugin blocks install and uninstall
  Test: cleaner_plugin_refuses_foreign_files
  Given the plugin destination contains content not owned by the embedded manifest
  When plugin install or uninstall is requested
  Then the command returns an error and preserves every foreign byte

Scenario: QML invokes only the Rust cleaner JSON contract
  Test: cleaner_plugin_uses_rust_json_commands
  Given the embedded Omarchy bar panel source
  When its Process commands are inspected
  Then scan, plan, and apply invoke `omarchy-rs cleaner` without a shell or privileged command

### Rule: evidence-gate — Measure value before default enablement

Scenario: Comparable cleaner benchmark report
  Mode: optimize
  Test: cleaner_benchmark_report_has_comparable_metrics
  Given the deterministic generated workspace and completed Python and Rust runs
  When the benchmark report is validated
  Then both implementations report identical fixture settings, three warmups, thirty samples, wall time, CPU time, peak RSS, I/O when available, and child-process count

Scenario: Candidate misses the performance threshold
  Test: cleaner_benchmark_gate_rejects_regression
  Given Rust improves neither median wall time nor peak RSS by 40% or regresses the other by more than 10%
  When benchmark eligibility is evaluated
  Then `default_enabled` is false with a threshold or regression reason

## Out of Scope

- Pacman cache, orphan packages, systemd journal, Snapper, Docker, and root-owned cleanup.
- Generic `build` or `dist` deletion and arbitrary user-entered deletion paths.
- Background scheduled cleanup, automatic deletion, or cleanup without a visible confirmation step.
- Replacing Quickshell or implementing the panel UI itself in Rust.
