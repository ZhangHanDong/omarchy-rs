# Workspace cleaner benchmark

This benchmark compares the release Rust scanner with the deterministic Python
reference in `benches/cleaner_reference.py`. Both implementations traverse the
same generated warm-cache tree and apply the same high-confidence Rust and Node
artifact rules.

## Workload

- Fixture: `workspace-v1`
- 12 Rust projects and 12 Node projects
- 1,000 files per build artifact, 24,000 generated artifact files total
- 128 bytes per generated file
- 3 warmups followed by 30 measured runs per implementation
- Process metrics sampled directly from `/proc/<pid>` every 500 microseconds
- CPU resolution follows the host `CLK_TCK`; peak RSS uses `VmHWM`

The benchmark uses an isolated temporary HOME and never reads or cleans the
real `~/Work`. The complete machine-readable result is in
[`cleaner-workspace-v1.json`](cleaner-workspace-v1.json).

## Result

| Metric | Python | Rust | Rust change |
|---|---:|---:|---:|
| Median wall time | 140.37 ms | 19.01 ms | 86.5% lower |
| Median CPU time | 130 ms | 10 ms | 92.3% lower |
| Peak RSS | 15,000 KiB | 2,988 KiB | 80.1% lower |
| Median filesystem read bytes | 0 | 0 | warm-cache run |
| Median filesystem written bytes | 0 | 0 | no writes |
| Child processes | 0 | 0 | equal |

The admission gate passed: Rust improved both primary metrics by more than 40%
and regressed neither by more than 10%. These results support enabling the
scanner in the plugin. They do not establish an energy claim because no direct
energy counter was measured.

## Reproduce

```bash
cargo build --release --locked --bin omarchy-rs
cargo bench --bench cleaner-benchmark --locked
```
