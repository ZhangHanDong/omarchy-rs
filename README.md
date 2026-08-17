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

## Engineering documents

- [Architecture](docs/architecture.md)
- [Compatibility model](docs/compatibility.md)
- [Benchmark policy](docs/benchmarking.md)
- [Roadmap](docs/roadmap.md)
- [Contributing](CONTRIBUTING.md)
- [Project Contract](specs/project.spec.md)
- [Agent Usage pilot Contract](specs/task-agent-usage-parity.spec.md)

Machine-consumable requirements and decisions live under `knowledge/` and are
validated with `agent-spec`.

## License

License selection is intentionally pending before implementation begins.
