# Roadmap

## Phase 0: contracts and harness

- Establish KLL requirements, architecture decisions, and project constraints.
- Pin an Omarchy baseline and build isolated differential-test helpers.
- Evaluate Agent Usage libraries through the dependency-admission Contract.
- Build a reproducible benchmark harness before selecting numeric thresholds.

## Phase 1: Codex Agent Usage pilot

- Characterize valid, empty, partial, malformed, cold, and warm fixtures.
- Implement the Codex collector behind an inactive compatibility adapter.
- Demonstrate parity, benchmark value, activation checks, and offline rollback.

## Phase 2: shared Agent Usage engine

- Extract a provider model only after the Codex implementation is stable.
- Evaluate Claude and Fireworks independently against their upstream behavior.
- Avoid forcing provider-specific formats into a false common abstraction.

## Phase 3: measured status-query candidates

- Profile periodically invoked power, battery, system, network, menu, and
  Hyprland queries.
- Create one requirement and task Contract per accepted candidate.
- Consider caching only after measuring invalidation and staleness requirements.

## Deferred decisions

- A top-level `omarchy` proxy.
- A resident daemon.
- Mutating or privileged commands.
- Upstream contribution of stable components.
