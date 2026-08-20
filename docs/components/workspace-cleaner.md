# Workspace Cleaner

Workspace Cleaner inventories regenerable build artifacts below a user-owned
workspace and removes only candidates selected through a persisted,
explicitly confirmed plan. The default root is `~/Work`.

## Architecture and release model

The feature has two coupled parts:

- `omarchy-rs cleaner` is the Rust engine and JSON CLI. It scans, classifies,
  measures, creates plans, revalidates candidates, and removes confirmed
  artifacts.
- `omarchy-rs.cleaner` is a user-owned Omarchy QML plugin. It displays the
  report and invokes the Rust CLI directly without a shell or privilege
  escalation.

The QML plugin cannot operate independently because it deliberately contains
no filesystem cleanup implementation. Although it can be installed and
uninstalled separately, it should be shipped with the same `omarchy-rs`
crate/CLI version. A separate plugin release would require a versioned public
JSON protocol and compatibility matrix; the current single-package model is
safer and simpler.

## Install

Install the CLI, install the embedded plugin into the user configuration, and
enable its bar widget:

```bash
cargo install omarchy-rs
omarchy-rs cleaner install-plugin
omarchy plugin enable omarchy-rs.cleaner
```

The plugin is installed under
`~/.config/omarchy/plugins/omarchy-rs.cleaner`. It never modifies
`/usr/share/omarchy`.

## Clean from the panel

1. Click the broom in the Omarchy bar. Opening the panel performs a read-only
   scan.
2. Review the canonical project names, artifact types, sizes, and eligibility.
3. Select the artifacts to remove. A crossed candidate has recent writes and
   cannot be selected yet.
4. Scroll to the bottom and choose **Review cleanup**. This persists an exact
   cleanup plan but does not remove files.
5. Check the planned total and choose **Confirm removal**.

The panel reports reclaimed bytes and any candidates skipped because they
changed between planning and confirmation.

## Broom status and settings

The broom is white below the warning threshold and uses Omarchy's urgent color
when the scanned regenerable total reaches the threshold. The default is 400
GiB. Configure the threshold or workspace root with the standard Omarchy bar
command:

```bash
omarchy bar set omarchy-rs.cleaner cleanupAlertGiB 600
omarchy bar set omarchy-rs.cleaner root ~/Work
```

`cleanupAlertGiB` accepts 1 through 10,000. The root must resolve to an
existing directory strictly below the current user's HOME. Changes are
applied by the Omarchy shell to the live widget.

## Command-line use

Scan is always read-only:

```bash
omarchy-rs cleaner scan --root ~/Work
omarchy-rs cleaner scan --root ~/Work --json
```

For automation, consume the JSON scan, choose candidate IDs, and create a
plan. Applying requires both the returned plan ID and its confirmation token:

```bash
omarchy-rs cleaner plan \
  --root ~/Work \
  --candidate <candidate-id> \
  --json

omarchy-rs cleaner apply \
  --plan <plan-id> \
  --confirm <confirmation-token> \
  --json
```

Do not parse the human-readable output for automation.

## What is eligible

The cleaner currently recognizes only high-confidence project artifacts:

- Rust `target` directories with a sibling `Cargo.toml` and Cargo build
  evidence.
- Node dependency/cache directories with a sibling `package.json` and the
  expected artifact layout.

Generic directories named `build`, `dist`, or `target` without project and
tool evidence are excluded.

## Safety guarantees

- Scan and plan never delete artifact contents.
- `/`, HOME itself, relative roots, missing roots, and roots outside HOME are
  rejected.
- Symlinks, `.git`, other devices, and content not owned by the HOME owner are
  not traversed.
- Apply removes only IDs recorded in the persisted plan.
- Every candidate is revalidated by canonical path, device, inode, owner,
  classification evidence, size, file count, and latest write time.
- Artifacts written within the last five minutes are skipped.
- No cleaner command uses `sudo`, `pkexec`, a package manager, or a shell to
  scan and remove files.

## Remove the plugin

Disable the bar widget and remove only plugin files owned by `omarchy-rs`:

```bash
omarchy plugin disable omarchy-rs.cleaner
omarchy-rs cleaner uninstall-plugin
```

Uninstall refuses to remove a foreign or locally modified plugin directory.
The CLI remains available after removing the panel.

## Performance evidence

The deterministic benchmark and raw measurements are documented in
[Workspace cleaner benchmark](../benchmarks/workspace-cleaner.md). The
benchmark uses an isolated synthetic HOME and never cleans the real
workspace.
