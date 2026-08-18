# Octoscode Agent Usage benchmark

Captured on 2026-08-19 (Asia/Shanghai). This report compares the installed
Python collector with the release-mode Rust canary on the same real local
ledger. It records only aggregate file counts and performance measurements;
no ledger content, model names, prompts, credentials, or usage totals are
retained here.

## Environment and inputs

| Item | Value |
| --- | --- |
| OS | Omarchy 4.0.0 |
| Kernel | Linux 7.1.8-arch1-3 x86_64 |
| CPU | Intel Core i7-11800H, 8 cores / 16 threads |
| Rust | rustc 1.97.0 |
| Python | Python 3.14.7 |
| omarchy-rs base commit | `b317aeaf310a63b884d5ab51897f0747bd2a88be` plus the uncommitted Octoscode change |
| Python SHA-256 | `d67554a97fd4c27bec3c1557f06fba4498aaebe949eb8836d7c145ce9a9b707a` |
| Rust SHA-256 | `72ddf731ff48a5800d9b5803db7533f2c864e695f08d8bc070aa14ade4c44015` |
| Python file size | 5,815 bytes, excluding the Python runtime |
| Rust binary size | 1,193,520 bytes |
| Input | 6 ledger files, 176,390 lines, 50,461,397 bytes |
| Cache state | Logical forced rescan; OS page cache warm and uncontrolled |

## Correctness and stability

The complete Python and Rust JSON records matched after removing only
`updatedAt` and `collectorBackend`. The Rust candidate then completed 400
additional real-ledger runs with zero failed exits and one normalized output
hash. Synthetic parity, malformed-input, repeated-turn, fallback, and atomic
write tests also passed through the agent-spec lifecycle.

## Wall-clock latency

The harness used 100 alternating AB/BA pairs. Each sample launches the real
executable and scans the full ledger. Timing uses `date +%s%N`; its launch
overhead is present on both sides. Prior invocations warmed executable and OS
page caches.

| Metric | Python | Rust | Rust improvement |
| --- | ---: | ---: | ---: |
| Minimum | 176.872 ms | 50.164 ms | 3.53x |
| Median | 180.117 ms | 51.857 ms | 3.47x |
| Mean | 181.644 ms | 52.443 ms | 3.46x |
| p95 | 191.283 ms | 56.687 ms | 3.37x |
| p99 | 204.179 ms | 59.040 ms | 3.46x |
| Maximum | 208.152 ms | 59.374 ms | 3.51x |

## CPU time

Shell accounting over 100 launches measured user and system CPU separately.

| Metric | Python | Rust | Change |
| --- | ---: | ---: | ---: |
| User CPU | 16.329 s | 4.033 s | -75.3% |
| System CPU | 1.648 s | 1.079 s | -34.5% |
| Total CPU | 17.977 s | 5.112 s | -71.6% |
| Total CPU per refresh | 179.77 ms | 51.12 ms | 3.52x less |

At the installed five-minute refresh interval (288 runs/day), this projects to
about 51.77 CPU-seconds/day for Python and 14.72 CPU-seconds/day for Rust, a
reduction of about 37.05 CPU-seconds/day. CPU time is only an energy proxy; no
package-energy counter was available, so this is not a direct energy claim.

## Peak memory

Ten alternating one-shot runs were measured through systemd's cgroup memory
accounting.

| Metric | Python | Rust | Change |
| --- | ---: | ---: | ---: |
| Median peak | 9.5 MiB | 2.0 MiB | -78.9% |
| Minimum peak | 9.4 MiB | 2.0 MiB | -78.7% |
| Maximum peak | 9.8 MiB | 2.2 MiB | -77.6% |

The initial Rust implementation loaded each ledger into one `String` and
measured 20.4--20.9 MiB. The corrected implementation uses `BufReader` with a
reused byte buffer and lossy UTF-8 conversion per line. That removes the
whole-file allocation and cuts measured peak RSS by about 78% versus Python
and about 90% versus the initial Rust implementation.

## Decision

The canary is justified by output parity, a 3.37x p95 wall-time improvement,
71.6% lower measured total CPU work, and 77.6% lower worst observed peak RSS.
It remains a guarded user overlay because the verified Python fingerprint is
still required for admission. Fingerprint drift, candidate validation failure,
or unsupported arguments retain the Python fallback.

This result applies to the recorded 50.5 MB real-ledger workload and warm OS
page cache. It does not claim cold-cache performance, direct energy savings,
or identical ratios on other machines.
