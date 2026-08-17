# Contributing

## Workflow

1. Add or update the governing requirement under `knowledge/requirements/`.
2. Write a bounded task Contract under `specs/` with deterministic test selectors.
3. Run `agent-spec parse` and `agent-spec lint --min-score 0.7`.
4. Implement against isolated synthetic fixtures.
5. Run the task lifecycle and the Rust test suite before claiming completion.
6. Attach compatibility and benchmark evidence to performance-related changes.

## Component admission

A proposed Rust port must identify a measured hot path, its upstream behavioral
surface, rollback mechanism, and operational risk. Short wrappers dominated by
external commands normally remain upstream Bash.

## Safety

Tests must not read real agent logs or credentials, modify the developer's
Omarchy installation, invoke pacman or sudo, or write outside their temporary
directories. Never weaken a Contract to make an implementation pass.
