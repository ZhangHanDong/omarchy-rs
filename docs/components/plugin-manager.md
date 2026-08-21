# Plugin Manager

The Plugin Manager is the unified control plane for QML plugins shipped by
omarchy-rs. It manages only Workspace Cleaner, Local Skill Manager, and Network
Inspector. Learn and Agent Usage use different integration mechanisms and are
intentionally not presented as plugins.

## Inspect health

```bash
omrs plugin list --json
omrs plugin doctor --json
```

Each record reports the component and canonical Omarchy id, installed and
enabled state, ownership validity, whether installed bytes match the current
binary, dependency readiness, and the omarchy-rs version. Doctor uses the same
machine-readable report and adds typed problems such as `stale-plugin`,
`missing-dependency:sniffnet`, or `ownership-error:owned-file-modified`.

## Install and enable

```bash
omrs plugin install cleaner
omrs plugin install skills
omrs plugin install network-inspector
omrs plugin enable cleaner
```

Valid component names are `cleaner`, `skills`, and `network-inspector`.
Network Inspector requires `sniffnet` to be installed. Enablement goes through
the public `omarchy plugin enable` command; the manager does not edit
`shell.json` itself.

## Update

Update every currently installed omarchy-rs plugin after upgrading the crate:

```bash
cargo install omarchy-rs --force
omrs install
omrs plugin update
```

Bulk update never installs an absent plugin. To update one installed component:

```bash
omrs plugin update skills
```

To restart the Omarchy shell immediately after a successful update, opt in
explicitly:

```bash
omrs plugin update --restart
omrs plugin update skills --restart
```

The restart executable runs only after all selected plugin files pass ownership
checks and the update completes. A failed update never restarts the shell.

Updates first verify every ownership receipt and installed file. Foreign,
missing, or locally modified files are refused rather than overwritten.

## Uninstall

```bash
omrs plugin uninstall network-inspector
```

Uninstall first calls `omarchy plugin disable` and then removes only files whose
hashes match the omarchy-rs ownership receipt. Extra or changed files stop the
operation.

## Apply shell changes

Mutation results contain `restartRecommended` and `restarted`. Without
`--restart`, the manager deliberately does not restart the desktop shell. After
installing, enabling, updating, or uninstalling plugins, run:

```bash
omarchy-restart-shell
```

This explicit restart avoids terminating an active shell session from an
unattended command or Agent workflow.

## Troubleshooting

If `omarchyAvailable` is false, confirm `omarchy` is available on `PATH`. Tests
and unusual installations can override the executable without invoking a shell:

```bash
export OMARCHY_RS_OMARCHY=/path/to/omarchy
omrs plugin doctor --json
```

Ownership errors require manual review. Do not delete or overwrite the reported
directory merely to silence the diagnostic; it may contain user changes.
