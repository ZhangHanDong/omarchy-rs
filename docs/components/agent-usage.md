# Agent Usage Rust overlay

The Agent Usage replacement is managed by the `omarchy-rs` CLI but executed by
separate Rust updater and collector binaries. Activation changes command
resolution only: it places the user-owned updater before Omarchy's updater in
`PATH`. It does not replace files in the Omarchy package.

```text
Omarchy Usage panel
        |
        v
omarchy-agent-usage-update (user PATH shim)
        |
        +-- Codex / Claude / Octoscode shadow
        |       |
        |       +-- eligible and valid ----------> Rust record
        |       `-- drifted or unsupported ------> absolute Python collector
        |
        `-- Grok native collector ---------------> Rust record
                        |
                        v
        ~/.local/state/omarchy/agents/usage/*.json
```

For Codex, Claude Code, and Octoscode, a shadow admits the Rust result only
when the installed upstream collector has the verified fingerprint and the
requested surface is supported. The Rust record must also pass schema and
provider validation. An upstream change, unsupported arguments or sources, or
an invalid candidate invokes the original collector by absolute path instead.
This decision is provider-local, so one provider can fall back without
changing the others.

The overlay adds one presentation field after validation:
`collectorBackend` is `rust` for an admitted canary record and `python` for an
upstream fallback. Providers written directly by Omarchy have no such field
and the optional user UI treats that absence as `python`.

The shadow receipt contains only compatibility field names:

```text
omarchy-rs-shadow {"differingFields":[],"localFieldsMatch":true,"schemaVersion":1}
```

It contains no token values, dates, model names, prompt content, or
credentials. The updater delegates unreplaced agents to the absolute installed
updater, invokes the enabled Rust provider binaries, validates their records,
and atomically replaces only the corresponding user-owned state files.

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

Grok has no stock Omarchy collector to shadow. Its native Rust collector scans
only `$GROK_HOME/sessions/**/updates.jsonl` (or `~/.grok`) and accepts structured
`turn_completed` usage records. It never reads prompt history, chat history,
responses, credentials, or telemetry. Grok has no Python fallback because this
is a new provider integration rather than a replacement; malformed and unknown
records are skipped and no rate limit or subscription data is invented.

## Quota windows for Grok and Octoscode

The panel distinguishes local usage totals from authoritative account quotas.
Local token totals cannot be converted into a Session, 5-hour, Weekly, or
Monthly percentage because provider pricing and allowance rules are not present
in the local records.

For a personal Grok or SuperGrok account, xAI currently exposes one shared
Weekly pool in **Grok → Settings → Usage**. The public Grok CLI does not expose
that percentage or reset time, so the Rust collector keeps `limits` empty and
shows `Weekly quota: Grok Settings → Usage`. It does not inspect browser cookies
or ask for an xAI Management Key. Management Keys belong to the separate xAI
Business/team API billing surface and do not represent a personal Grok
subscription.

Octoscode can use different providers and models. Its public local ledger
contains model names and token totals, but no authoritative provider quota
snapshot. The Rust collector therefore keeps `limits` empty and shows
`Provider quotas unavailable`. If Octoscode later exposes a public, typed quota
snapshot, individual provider/model windows can be added without inferring them
from token totals.

## User overlay deployment

`omarchy-rs install` installs the updater and provider shadows under
`~/.local/share/omarchy-rs/libexec`; `activate agent-usage` creates the single
updater shim under the adjacent `bin` directory. The CLI refuses activation
unless that directory precedes `/usr/share/omarchy/bin` in PATH. No file under
`/usr/share/omarchy` is modified.

The activation record enables Codex, Claude Code, Octoscode, and Grok. The
updater passes canary modes to the first three provider shadows; their original
collectors remain installed for fallback. Grok is routed to the native sibling.

## Offline rollback

Run `omarchy-rs rollback agent-usage`, then restart the Omarchy shell if its
process predates the PATH change. The retained PATH entry contains no matching
updater and command resolution falls through to the official updater. Rollback
needs no network access and does not reconstruct or edit an Omarchy package
file.

The `OMARCHY_RS_CODEX_MODE=canary` route returns Rust only for the exact
verified upstream fingerprint,
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
