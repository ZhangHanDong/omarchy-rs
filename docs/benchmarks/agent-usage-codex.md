# Codex Agent Usage benchmark protocol

This benchmark decides whether the Rust collector is worth shipping. Passing
correctness tests is necessary but does not establish value, and a parser-only
microbenchmark cannot authorize replacement of the end-to-end Omarchy command.

## Comparison

Compare the pinned `omarchy-agent-usage-codex` executable with the release-mode
`omarchy-rs agent-usage-codex` executable. Both receive the same isolated HOME,
PATH, state directories, clock policy, fake app-server, and versioned fixtures.
No benchmark reads real agent data or credentials.

Run small, typical, large, malformed, unchanged-warm-cache, and single-append
fixtures. Logical cold means an empty application cache; warm means the
versioned cache produced by the preceding compatible run. Do not describe OS
page-cache state as cold unless the harness controls and records it.

Each case uses 10 warm-ups and at least 100 interleaved measured samples. The
report retains raw samples and publishes median, p95, dispersion, and 95%
confidence intervals for wall and CPU time, plus maximum RSS, available I/O,
page faults, context switches, and child-process count.

## Admission gates

Default activation requires all of:

- CPU time at least 30% lower;
- p95 wall time at least 20% lower;
- maximum RSS no more than 20% higher;
- bytes read no more than 10% higher when measurable;
- fixture parity and all stability checks passing.

A strong performance claim additionally requires CPU time at least 50% lower,
2x wall-time speedup, or at least 40% lower daily CPU work at Omarchy's refresh
interval. Energy is reported only when package-energy counters are available;
otherwise CPU time is explicitly a proxy.

The stability campaign runs 1,000 valid and malformed invocations, compares
normalized output hashes, records every exit status, and continuously reads the
state file during writes. Any crash, panic, nondeterministic valid output, or
partial JSON rejects default activation.

## Preliminary upstream baseline

The first typical-fixture run is retained in
`codex-upstream-typical-100.json`: 100 synthetic sessions, 10 warm-ups, and 100
forced-rescan samples. It measured 48.39 ms median wall time, 51.73 ms p95,
40 ms median sampled CPU time, and 25,180 KiB peak RSS. All exits succeeded and
all normalized output hashes matched. This is a harness bring-up baseline, not
a Rust performance claim; environment identity and candidate A/B interleaving
must be added before it can authorize activation.

## Recorded A/B results

The reproducible raw reports are:

- `codex-ab-typical-100.json`: 100 alternating AB/BA pairs over 100 valid
  synthetic session files.
- `codex-ab-malformed-100.json`: 100 alternating AB/BA pairs over 100
  malformed synthetic session files.
- `codex-rs-stability-valid-1000.json`: 1,000 candidate invocations over the
  valid fixture.
- `codex-rs-stability-malformed-1000.json`: 1,000 candidate invocations over
  the malformed fixture.

On the recorded i7-11800H Omarchy host, the valid-fixture upstream median and
p95 wall times were 55.85 ms and 61.02 ms, versus 1.58 ms and 2.35 ms for the
Rust candidate. Peak sampled RSS was 25,464 KiB upstream and 3,096 KiB for the
candidate. The malformed-fixture figures were 54.06/57.15 ms and 25,424 KiB
upstream versus 1.47/2.15 ms and 3,116 KiB for Rust. Normalized output hashes
matched in both A/B reports.

All 1,000 valid and 1,000 malformed candidate invocations exited successfully,
and each campaign produced one normalized output hash. Their p95 wall times
were 1.91 ms and 1.70 ms respectively.

These results pass the wall-time and RSS gates by a wide margin. Linux process
CPU accounting on this host has a 10 ms tick, so the sub-tick candidate samples
record zero CPU ticks. That establishes a conservative upper bound but is not a
precise CPU measurement; a higher-resolution CPU counter is still required
before publishing an exact CPU-reduction percentage. Atomic state-file reader
tests also remain pending because this candidate currently implements the local
collector output, not the cache/shim activation layer.
