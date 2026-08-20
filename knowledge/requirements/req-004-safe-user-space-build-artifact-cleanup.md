---
kind: requirement
id: REQ-004
title: "Safe user-space build artifact cleanup"
status: accepted
liveness: auto
tags: []
---

## Problem

Large Rust and Node workspaces accumulate gigabytes of regenerable build
artifacts, but directory names such as `target`, `build`, and `dist` are not by
themselves proof that content is safe to delete. Users need a fast inventory
and an explicit cleanup workflow that cannot escape approved user-owned roots
or silently turn a scan into deletion.

## Requirements

[REQ-004-SCAN] The cleaner MUST scan configured user-owned roots, defaulting to `~/Work`, and report byte size, file count, artifact kind, project root, and the filesystem evidence used to classify each candidate.

[REQ-004-CLASSIFY] The cleaner MUST classify Rust `target` and Node `node_modules`, `.next/cache`, `.turbo`, and `.vite` artifacts only when language-specific project markers validate them; generic `build` and `dist` directories MUST remain excluded.

[REQ-004-BOUNDARY] The cleaner MUST NOT follow symbolic links, cross the configured scan root, cross filesystem boundaries, inspect `.git` contents, or delete a path outside an explicit user-owned cleanup plan.

[REQ-004-PLAN] Scanning MUST be read-only, and cleanup MUST require a persisted plan containing exact candidate identities plus an explicit confirmation token.

[REQ-004-RACE] Before deleting each candidate, the cleaner MUST revalidate its canonical parent, ownership, file type, device and inode identity, project evidence, and recent-write guard; a changed or unverifiable candidate MUST be skipped and reported.

[REQ-004-OBSERVE] Machine-readable scan, plan, and apply results MUST distinguish reclaimed bytes, skipped candidates, failed candidates, and reasons without claiming bytes from overlapping paths twice.

## Scenarios

Rule: REQ-004-SCAN
Scenario: Validated workspace inventory
  Given a synthetic Work tree containing Rust and Node projects
  When the cleaner scans the tree
  Then every reported candidate includes size, file count, kind, project root, and classification evidence

Rule: REQ-004-CLASSIFY
Scenario: Ambiguous directory names remain untouched
  Given source-owned `target`, `build`, and `dist` directories without valid artifact evidence
  When the cleaner scans and plans the tree
  Then none of those directories appears in the cleanup plan

Rule: REQ-004-BOUNDARY
Scenario: Filesystem boundaries are contained
  Given symlinks, a `.git` directory, an outside-root target, and a foreign-filesystem boundary
  When the cleaner scans the configured root
  Then it reads and plans no descendant through those boundaries

Rule: REQ-004-PLAN
Scenario: Scan cannot delete
  Given a tree containing validated build artifacts
  When scan completes without a cleanup plan and confirmation token
  Then every artifact remains byte-for-byte present

Scenario: Confirmed plan deletes only selected candidates
  Given a persisted plan selecting one of two validated candidates
  When apply receives the matching confirmation token
  Then only the selected candidate is removed and the result reports its reclaimed bytes

Rule: REQ-004-RACE
Scenario: Replaced candidate is skipped
  Given a persisted plan whose candidate is replaced or modified after planning
  When apply revalidates the plan
  Then the candidate remains present and the result reports an identity or recent-write rejection

Rule: REQ-004-OBSERVE
Scenario: Nested candidates are counted once
  Given a `node_modules` tree containing a nested `.vite` cache
  When scan and apply results are generated as JSON
  Then totals count the outer selected artifact once and expose deterministic skip and failure reasons

## Dependencies

- REQ-002

## Source Trace

- User direction on 2026-08-20: add an Omarchy cleanup plugin backed by Rust,
  prioritizing Rust and npm build artifacts under `~/Work`.
- Local Omarchy plugin contract: `/usr/share/omarchy/shell/plugins/agents` uses
  QML `Process` with machine-readable output from an external command.

## Open Questions

None.

## Next

Compile this requirement into the Rust workspace cleaner and Omarchy shell
plugin task contract.
