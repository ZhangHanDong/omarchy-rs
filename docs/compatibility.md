# Compatibility model

Compatibility is defined by observable behavior, not implementation structure.
Each replacement declares which of these surfaces apply:

- accepted arguments and environment variables;
- stdout and stderr content and separation;
- exit status, including partial failures;
- JSON schema, omission rules, and ordering where observable;
- files read and written, permissions, and atomicity;
- cold and warm state behavior;
- fallback and command-precedence behavior.

## Baseline

The baseline is a pinned Omarchy commit plus versioned synthetic fixtures. Tests
run the upstream and Rust implementations with isolated `HOME`, `PATH`, state,
and configuration directories. Normalization may remove nondeterministic fields
such as temporary paths or timestamps, but it must be documented per component.

## Drift

The compatibility database records the upstream executable identity used by the
last passing differential test. A changed identity produces `unverified`, not
`incompatible`. Default activation is withheld until the component's Contract
passes against the new baseline.

## Rollback

Rollback is local and offline. It removes or disables only the selected shim and
then verifies that command resolution reaches the already-installed upstream
executable. It never downloads or reconstructs an upstream script.

## Non-claims

A PATH shim does not intercept callers that use an absolute executable path.
`omarchy-rs doctor` must distinguish interactive, graphical-session, sudo, and
absolute-path resolution rather than reporting a single global status.
