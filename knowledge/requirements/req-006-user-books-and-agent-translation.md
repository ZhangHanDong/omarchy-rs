---
kind: requirement
id: REQ-006
title: "User books and guarded Agent translation"
status: accepted
liveness: auto
tags: [learn, books, translation, agents, omarchy]
---

## Problem

Omarchy's Learn menu is a static collection of web links. Users need to add
their own books without editing package files and translate an individual
public chapter through an installed coding Agent without turning Learn into an
unbounded crawler or exposing local Agent state.

## Requirements

[REQ-006-REGISTRY] The system MUST maintain a versioned user Book registry under `$XDG_CONFIG_HOME/omarchy-rs/learn`, validate stable ids, labels, and HTTPS public URLs, and provide deterministic add, list, and remove JSON commands.

[REQ-006-MENU] The system MUST add custom Book and Agent Translate entries through the watched user Omarchy menu extension, preserve all foreign JSONC bytes outside an exact owned marker block, and MUST NOT modify `/usr/share/omarchy`.

[REQ-006-FETCH] Translation MUST fetch exactly one configured HTTPS page, reject credentials, IP literals, localhost and non-text responses, enforce bounded redirect/content/time limits, never crawl linked pages, and permit reserved proxy Fake-IP answers only while their local tunnel interface is active.

[REQ-006-PLAN] Agent invocation MUST require a persisted exact plan and confirmation token, revalidate the Book registry identity before execution, and expose the selected Agent, language, URL, byte bound, and cache target before confirmation.

[REQ-006-AGENTS] Codex, Claude Code, and Grok MUST be invoked directly with non-interactive, non-persistent, no-write or no-tool arguments; an Agent without a safe public single-turn interface, including current Octoscode CLI, MUST be reported unavailable.

[REQ-006-OUTPUT] Translation output MUST be bounded, cached by source identity plus Agent and language, written atomically below `$XDG_CACHE_HOME/omarchy-rs/learn`, retain source attribution, and be openable as a local escaped HTML document.

[REQ-006-PRIVACY] The implementation MUST NOT inspect Agent logs, sessions, prompts, credentials, profiles, databases, or unrelated local documents and MUST NOT include fetched source text in plans, receipts, logs, or test output.

## Scenarios

Rule: REQ-006-REGISTRY
Scenario: A valid user Book round-trips deterministically
  Given an isolated config root and one valid HTTPS Book
  When add, list, and remove execute
  Then JSON changes deterministically and no Omarchy package path changes

Rule: REQ-006-MENU
Scenario: Menu synchronization preserves foreign JSONC
  Given a user menu containing comments and foreign entries
  When Learn entries are synchronized and later removed
  Then only the exact omarchy-rs marker block changes

Rule: REQ-006-FETCH
Scenario: Unsafe or oversized sources fail closed
  Given local, credentialed, redirected-private, non-text, and oversized sources
  When a chapter is prepared
  Then each is rejected before Agent invocation

Rule: REQ-006-PLAN
Scenario: Translation requires fresh exact confirmation
  Given a persisted plan
  When its token is wrong or the Book registry changes
  Then no Agent starts and no translation cache is written

Rule: REQ-006-AGENTS
Scenario: Supported Agents use safe direct argv
  Given synthetic Codex, Claude, Grok, and unavailable Octos executables
  When confirmed translations execute
  Then direct argv is bounded and Octos reports unavailable without private-state access

Rule: REQ-006-OUTPUT
Scenario: Translation output is escaped and cached
  Given successful synthetic Agent output containing markup
  When the result is persisted
  Then escaped local HTML includes attribution and a repeat request returns the cache

Rule: REQ-006-PRIVACY
Scenario: Reports exclude source and private sentinels
  Given synthetic page, credential, prompt, session, and log sentinels
  When plan and result JSON are serialized
  Then neither source text nor private sentinels appears

## Dependencies

- REQ-001
- REQ-003

## Source Trace

- User confirmation on 2026-08-21: implement custom Learn books and Agent translation with SDD.
- User-approved design on 2026-08-20: use a user Book registry, Learn integration, single-chapter translation, local cache, Agent selection, and no whole-book crawl.
- Omarchy 4.0 menu source: user extensions are watched at `~/.config/omarchy/extensions/omarchy-menu.jsonc` and merged over package defaults.
- Installed Agent CLI inspection on 2026-08-21: Codex, Claude Code, and Grok expose non-interactive modes; Octos exposes interactive chat and ACP but no equivalent bounded single-turn CLI.
- User-confirmed fix on 2026-08-21: support Mihomo Fake-IP DNS without weakening rejection of ordinary local and private destinations.

## Open Questions

None.

## Next

Implement the versioned Learn registry, owned menu block, guarded translation plan, direct Agent adapters, cache, tests, and user guide.
