# Benchmark policy

Performance work begins with a hypothesis and baseline. A port is not accepted
because it is written in Rust.

## Required report fields

- upstream commit and executable identity;
- omarchy-rs commit and build profile;
- CPU, kernel, Rust toolchain, and relevant dependency versions;
- fixture version, size, record count, and cold or warm state;
- warm-up count, measured sample count, and aggregation method;
- wall-clock time, CPU time, maximum RSS, and child-process count;
- bytes read and written when the platform exposes reliable counters.

Raw samples and the aggregation command must be retained. Reports contain no
real prompts, credentials, access tokens, or user telemetry.

## Eligibility

Every task Contract names primary metrics, numeric activation thresholds, and
resource-regression limits after the baseline harness has run. A statistically
noisy or operationally insignificant difference is not an improvement.

For periodically invoked tools, evaluation also reports work per refresh and
estimated daily work at the upstream refresh interval. Energy claims require an
energy measurement; CPU-time reduction alone must be described as a proxy.

## Correctness before speed

The same fixture used for performance measurement must first pass compatibility
tests. Faster output with different semantics is a failed replacement.
