<p align="center">
  <img src="https://raw.githubusercontent.com/Omarchy-rs/omarchy-rs/main/themes/rust-lang/backgrounds/rust-forge-4k.png" alt="Rust Forge — an unofficial Rust-inspired Omarchy theme" width="100%">
</p>

<p align="center"><em>Rust Forge — an original, unofficial Rust-inspired Omarchy theme.</em></p>

# omarchy-rs

`omarchy-rs` is a reversible Rust acceleration layer for selected
[Omarchy](https://omarchy.org/) user-space tools. It does not replace Omarchy as
a distribution and does not take ownership of official package files.

The project ports only measured hot paths where a Rust implementation can
reduce process creation, parsing overhead, repeated I/O, or failure ambiguity.
Every replacement must preserve its declared CLI and file-format behavior,
remain independently disableable, and leave the upstream implementation
installed as a fallback.

## Current status

The first replacement experiment is operational. The Agent Usage overlay has
native Rust collectors for Codex, Claude Code, Octoscode, and Grok; verified
Codex, Claude Code, and Octoscode integrations retain their installed Python
collectors as fallbacks. Grok is a native addition because stock Omarchy has no
Grok collector.

Install the release from crates.io, then install and activate the user overlay:

```bash
cargo install omarchy-rs
omarchy-rs doctor
omarchy-rs install
omarchy-rs activate agent-usage
omarchy-rs status
omarchy-rs --version
```

After `omarchy-rs install`, the user overlay also provides the short `omrs`
command. Both names execute the same binary, so new interactive commands can
use `omrs` while existing scripts keep using `omarchy-rs`:

```bash
omrs doctor
omrs learn books --json
omrs cleaner scan --root ~/Work --json
```

The overlay lives under `~/.local/share/omarchy-rs` and does not modify files
owned by the Omarchy package. Use `omarchy-rs rollback agent-usage` for an
offline rollback to the official commands.

The CLI manages the overlay; provider-specific Rust binaries perform the
collection. They preserve Omarchy's existing state-file contract, and the
panel reads `collectorBackend` to show whether the latest result came from
Rust or the Python fallback. See [Agent Usage](docs/components/agent-usage.md)
and [deployment](docs/deployment.md) for the complete data flow and rollback
rules.

The workspace cleaner is available as a guarded Rust engine and user-owned
Omarchy panel. It scans `~/Work` by default, recognizes only project-validated
Rust and Node build artifacts, and requires a persisted confirmation plan
before removal:

```bash
omarchy-rs cleaner install-plugin
omarchy plugin enable omarchy-rs.cleaner
omarchy-rs cleaner scan --root ~/Work --json
```

Click the broom in the Omarchy bar, select candidates, choose **Review
cleanup**, and then choose **Confirm removal**. The broom stays white by
default and highlights only when the regenerable total reaches the configured
threshold (400 GiB by default):

```bash
omarchy bar set omarchy-rs.cleaner cleanupAlertGiB 600
omarchy bar set omarchy-rs.cleaner root ~/Work
```

The QML plugin is a presentation layer for `omarchy-rs cleaner`; it does not
scan or remove files independently. Install and release the plugin with the
same `omarchy-rs` crate/CLI version so its JSON command contract cannot drift.
See the [Workspace Cleaner guide](docs/components/workspace-cleaner.md) for
installation, safety behavior, CLI use, configuration, and removal, and the
[cleaner benchmark](docs/benchmarks/workspace-cleaner.md) for the reproducible
Python/Rust comparison.

The Local Skill Manager treats `~/.agents/skills` as the portable source and
shows activation across Claude Code, Codex, Grok, and Octoscode. Claude Code
and Codex receive manager-owned links, Grok uses its native `.agents`
discovery, and Octoscode is changed only through its public `skills` command:

```bash
omarchy-rs skills install-plugin
omarchy plugin enable omarchy-rs.skills
omarchy-rs skills scan --json
```

The panel never reads Skill instruction bodies or Agent credentials, logs,
prompts, sessions, databases, or profiles. A sync or cancellation requires a
persisted review plan and confirmation token, and foreign destinations are
left untouched. See the [Local Skill Manager guide](docs/components/local-skill-manager.md).

Custom Learn Books and guarded single-chapter Agent translation are available
through the same CLI. The integration preserves the user's existing Omarchy
menu extension and leaves package-owned Learn entries unchanged:

```bash
omarchy-rs learn add --id rust-book --label "The Rust Book" \
  --url "https://doc.rust-lang.org/book/"
omarchy-rs learn sync-menu
```

The generated **Learn → Agent Translate** submenu supports confirmed Codex,
Claude, and Grok translation with bounded HTTPS fetching and escaped local
HTML caching. It supports public mdBook/GitHub Pages URLs and Mihomo Fake-IP
DNS, while retaining private-address and redirect protections. See the
[Learn Books guide](docs/components/learn-books.md).

## Rust Lang Omarchy theme

The Rust-inspired theme shown above is bundled under
[`themes/rust-lang`](themes/rust-lang/README.md). It includes the color palette,
Hyprland borders, icon selection, and original 4K Rust Forge wallpaper. Install
it as a user theme without modifying system-owned Omarchy files.

## Engineering documents

- [Architecture](docs/architecture.md)
- [Compatibility model](docs/compatibility.md)
- [Benchmark policy](docs/benchmarking.md)
- [Dependency policy](docs/dependency-policy.md)
- [Roadmap](docs/roadmap.md)
- [Contributing](CONTRIBUTING.md)
- [Project Contract](specs/project.spec.md)
- [Agent Usage pilot Contract](specs/task-agent-usage-parity.spec.md)
- [Workspace Cleaner Contract](specs/task-workspace-cleaner-plugin.spec.md)
- [Local Skill Manager Contract](specs/task-local-skill-manager-plugin.spec.md)
- [Learn Books Contract](specs/task-learn-books-agent-translation.spec.md)

Machine-consumable requirements and decisions live under `knowledge/` and are
validated with `agent-spec`.

## License

`omarchy-rs`, including the bundled Rust Lang theme and original Rust Forge
artwork, is distributed under the [MIT License](LICENSE).
