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

## Planned workspace

- `omarchy-cli`: multicall entry point, diagnostics, and explicit fallback.
- `omarchy-compat`: upstream discovery, fingerprints, activation, rollback,
  precedence probes, and eligibility decisions.
- `omarchy-agents`: Agent Usage collectors and shared parsing model.
- Additional domain crates are created only after a benchmark identifies a hot
  path and a task Contract defines compatibility.

## Deployment model

The canonical binary is planned for `/usr/local/lib/omarchy-rs/`. Selected
shims live in `/usr/local/bin/` only after a probe proves they precede the
official command in the target session. Packaging must not claim or overwrite
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
