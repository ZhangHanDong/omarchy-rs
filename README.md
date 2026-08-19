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
```

The overlay lives under `~/.local/share/omarchy-rs` and does not modify files
owned by the Omarchy package. Use `omarchy-rs rollback agent-usage` for an
offline rollback to the official commands.

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

Machine-consumable requirements and decisions live under `knowledge/` and are
validated with `agent-spec`.

## License

`omarchy-rs`, including the bundled Rust Lang theme and original Rust Forge
artwork, is distributed under the [MIT License](LICENSE).
