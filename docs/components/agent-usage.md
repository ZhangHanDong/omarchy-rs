# Agent Usage shadow overlay

The first live deployment mode is deliberately non-authoritative. The
`omarchy-agent-usage-codex-shadow` executable runs the installed absolute
Python collector, independently computes the Rust record, and compares the
documented local and app-server RPC fields. It always returns the upstream
stdout and exit status. Candidate failures and differences therefore cannot
change the panel record.

The overlay adds one presentation field after validation:
`collectorBackend` is `rust` for an admitted canary record and `python` for an
upstream fallback. Providers written directly by Omarchy have no such field
and the optional user UI treats that absence as `python`.

The shadow receipt contains only compatibility field names:

```text
omarchy-rs-shadow {"differingFields":[],"localFieldsMatch":true,"schemaVersion":1}
```

It contains no token values, dates, model names, prompt content, or
credentials. The updater adapter delegates all non-Codex agents to the
absolute installed updater, invokes the shadow collector for Codex, validates
the upstream record, and atomically replaces only the user-owned Codex state
file.

## Local test deployment

This machine installs the two release binaries under
`~/.local/share/omarchy-rs/bin`. The user Hyprland configuration prepends that
directory to the graphical-session `PATH`; no file under
`/usr/share/omarchy` is modified. Activation is valid only when the Quickshell
process resolves `omarchy-agent-usage-update` from this overlay directory.

## Offline rollback

Remove or rename
`~/.local/share/omarchy-rs/bin/omarchy-agent-usage-update`, then restart the
Omarchy shell. The retained PATH entry contains no matching updater and command
resolution falls through to
`/usr/share/omarchy/bin/omarchy-agent-usage-update`. Removing the companion
shadow binary is optional after the updater is disabled. Rollback needs no
network access and does not reconstruct or edit an Omarchy package file.

Shadow success is evidence for local-stat parity, not permission to return the
Rust record. Direct replacement remains blocked until app-server limits,
Pi/OMP, OpenCode, cache behavior, activation eligibility, and rollback tests
all pass the task Contract.

The optional `OMARCHY_RS_CODEX_MODE=canary` path is narrower than general
activation. It returns Rust only for the exact verified upstream fingerprint,
when Pi/OMP and OpenCode sources are absent, `--limits-only` was not requested,
and the Rust app-server probe did not report a protocol failure. Every rejected
condition executes the absolute Python collector. Shadow remains the default.
