# Agent Usage canary overlay

The first Codex deployment mode is deliberately non-authoritative. The
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
credentials. The updater adapter delegates unreplaced agents to the absolute
installed updater, invokes provider-specific canaries for Codex and Claude,
validates each record, and atomically replaces only the corresponding
user-owned state file.

Claude's canary scans native Claude Code transcripts in Rust, reads the
aggregate cache/history fallback, and probes the fixed Anthropic OAuth usage
endpoint without logging or persisting the access token. It returns Rust only
for the verified upstream fingerprint with no Pi, OMP, or OpenCode source and
no unsupported flag. Every unverified surface invokes the absolute Python
collector and marks the record `collectorBackend=python`.

Octoscode's canary scans its local `ui-protocol` ledger files in Rust and
preserves the upstream collector's last-completed-event-per-turn aggregation.
It is admitted only for the verified Python collector fingerprint. A changed
upstream, an invalid Rust record, or an unsupported argument executes the
original absolute Python collector instead. State writes use an atomic rename.

## User overlay deployment

`omarchy-rs install` installs the updater and provider shadows under
`~/.local/share/omarchy-rs/libexec`; `activate agent-usage` creates the single
updater shim under the adjacent `bin` directory. The CLI refuses activation
unless that directory precedes `/usr/share/omarchy/bin` in PATH. No file under
`/usr/share/omarchy` is modified.

The activation record enables Codex, Claude Code, and Octoscode. The updater
passes their canary modes to sibling shadows; original collectors remain
installed for fallback.

## Offline rollback

Run `omarchy-rs rollback agent-usage`, then restart the Omarchy shell if its
process predates the PATH change. The retained PATH entry contains no matching
updater and command resolution falls through to the official updater. Rollback
needs no network access and does not reconstruct or edit an Omarchy package
file.

Shadow success is evidence for local-stat parity, not permission to return the
Rust record. Direct replacement remains blocked until app-server limits,
Pi/OMP, OpenCode, cache behavior, activation eligibility, and rollback tests
all pass the task Contract.

The optional `OMARCHY_RS_CODEX_MODE=canary` path is narrower than general
activation. It returns Rust only for the exact verified upstream fingerprint,
when Pi/OMP and OpenCode sources are absent, `--limits-only` was not requested,
and the Rust app-server probe did not report a protocol failure. Every rejected
condition executes the absolute Python collector. Claude is independently
controlled by `OMARCHY_RS_CLAUDE_MODE=canary`; either provider can fall back
without changing the other.

Octoscode is independently controlled by
`OMARCHY_RS_OCTOSCODE_MODE=canary`. On the local real-ledger benchmark (100
warm invocations), Rust took 5.12 seconds versus Python's 18.06 seconds and
used 2.0--2.2 MiB peak memory versus Python's 9.4--9.8 MiB. The streaming Rust
collector therefore reduces measured CPU work by 71.6% and worst observed peak
memory by 77.6% on the recorded 50.5 MB ledger workload.
