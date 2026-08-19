# Architecture

## System position

`omarchy-rs` sits above the operating system and beside the official Omarchy
installation. It replaces only selected user-space command implementations.

```text
Quickshell / user / Omarchy scripts
                |
        selected command shim
          /              \
compatible             disabled, drifted,
and eligible           or unsupported
    |                         |
omarchy-rs             official executable
multicall binary       at an absolute path
```

Official Omarchy remains responsible for the desktop, system configuration,
updates, migrations, and packages. `omarchy-rs` must not write into paths owned
by `omarchy` or `omarchy-settings`.

## Package structure

crates.io exposes one `omarchy-rs` package. Its source remains grouped by
responsibility: lifecycle management, upstream compatibility, and Agent Usage
collectors compile as internal modules, while six explicit binary targets keep
the installed command surface stable. `dependency-probe` is a workspace-only
development package and is never part of the published dependency graph.

New functionality starts as an internal module. A separate public crate is
introduced only when it has an independent consumer and API, not merely to
mirror an implementation directory.

## Deployment model

The initial canonical binaries live under
`$XDG_DATA_HOME/omarchy-rs/libexec`. Selected shims live under the adjacent
`bin` directory only after a probe proves it precedes the official command in
the target session. A future system-wide package may use
`/usr/local/lib/omarchy-rs`, but packaging must never claim or overwrite
`/usr/bin/omarchy*`, `/usr/share/omarchy/**`, or `/etc/omarchy.conf`.

Absolute invocations of `$OMARCHY_PATH/bin/...` bypass PATH overlays. Supporting
those callers requires an explicit upstream integration or caller-specific
adapter and is not silently claimed as compatible.

## Failure model

Read-only replacements may fail open to a resolved absolute upstream command.
Mutating commands must never be retried automatically because the first attempt
may already have changed state. The initial phase excludes mutating and
privileged commands entirely.

## Compatibility ownership

Each component owns versioned synthetic fixtures, normalized differential
tests, an upstream fingerprint, and a documented compatibility surface. An
upstream change marks compatibility unverified but never blocks an Omarchy
upgrade.
