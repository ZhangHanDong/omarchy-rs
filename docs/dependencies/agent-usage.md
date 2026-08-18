# Agent Usage dependency evaluation

Evaluated on 2026-08-18 against Omarchy revision
`f32ebbdb730c4e8fe11e4046cef4267e466264ea`. All behavior probes use the
synthetic files under `fixtures/agent_usage`; no candidate was executed against
a real home directory, credential store, or provider endpoint.

## Decision

| Candidate | Pin | Boundary | Outcome |
| --- | --- | --- | --- |
| `tokenusage` | 1.5.2 | Local parsers plus unconditional HTTP and credential code | Adapt |
| ZhangHanDong/ccusage Rust adapters | `302fa5eaf61f7d09a8a2710be0c8fafbc2723e4c` | Pinned offline fork with deterministic pricing, configurable compatibility filters, an explicit-path Claude loader, and license metadata | Selected adaptation |
| `claude-usage` | 0.2.3 | Credential-backed Anthropic OAuth request | Isolate |

No unmodified candidate is directly accepted as a production dependency. The
selected parser basis is the ZhangHanDong ccusage fork pinned to an immutable
revision. Its `models-dev-pricing-only` patch gives Cargo consumers a
deterministic offline build without a LiteLLM environment snapshot or download.
All access is hidden behind an omarchy-rs backend so upstream API changes remain
localized. `tokenusage` remains a fallback only after its parser is separated
from provider HTTP and credential code behind disabled-by-default features.

`claude-usage` solves a different problem. It may be reconsidered for an
explicitly enabled online quota feature, but it is not a local usage parser.

## Omarchy compatibility baseline

The pinned Codex collector parses live and archived Codex sessions, merges Pi
and OpenCode usage, maintains cache/state semantics, and asks `codex
app-server` for account and rate-limit data. The Claude collector similarly
merges local transcripts, Pi and OpenCode records, cache data, and optional
Anthropic quota results. A library's normalized token totals therefore cover
only part of the public behavior.

The fixture matrix represents valid, empty, malformed, duplicate, cold-cache,
and warm-cache inputs. It deliberately contains fake identifiers and no prompt
text or secret-like values.

## Security and feature evidence

`tokenusage` declares `cli` as its only default feature. Disabling it removes
the large CLI/TUI/GUI stack, but `reqwest` is still unconditional and
`pipeline/official.rs` still compiles OAuth credential read/write and provider
HTTP clients. Its declared MSRV is 1.87, above this project's current 1.85.

The ccusage Codex and Claude adapters contain no network, credential, or unsafe
surface found by source review. The fork keeps their `0.0.0`, unpublished API
behind an internal backend and pins the entire Git graph. The core build script
remains, but `models-dev-pricing-only` makes its input entirely repository-local.
Fork provenance and replayable patch paths are recorded in `ccusage-fork.json`.

`claude-usage` reads Claude credentials from the Linux credentials file or the
macOS Keychain and calls Anthropic's OAuth usage endpoint. Disabling its
`blocking` default feature does not turn it into a local parser.

The exact machine-readable admission records are in
`docs/dependencies/agent-usage.json`. The optional crates.io pins in
`dependency-probe` exist only to resolve security and license metadata; their
features are disabled and no candidate code is called by the tests.

## Reproduction

```bash
cargo metadata --locked --format-version 1
cargo tree -e features
cargo tree --duplicates
cargo audit
cargo deny check
cargo test -p dependency-probe
```

On 2026-08-19, `cargo metadata --locked`, both tree inspections, and
`cargo audit` completed successfully over the 215-package workspace graph.
`cargo tree --duplicates` reports `windows-sys` 0.52 and 0.61 through
`terminal_size`; `cargo deny` records this as the configured duplicate warning,
not a security or license failure. Its advisories, bans, licenses, and sources
checks are all OK. The selected ccusage fork is resolved from its exact Git
revision and included in the locked audit and license graph.
