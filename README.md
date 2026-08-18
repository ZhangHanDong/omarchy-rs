<p align="center">
  <img src="themes/rust-lang/backgrounds/rust-forge-4k.png" alt="Rust Forge — an unofficial Rust-inspired Omarchy theme" width="100%">
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

Engineering and compatibility contracts are being established. No Omarchy
command is implemented or replaced yet. The first planned pilot is the
read-only Agent Usage collector, beginning with Codex fixtures.

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
