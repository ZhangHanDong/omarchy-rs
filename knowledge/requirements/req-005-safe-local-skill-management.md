---
kind: requirement
id: REQ-005
title: "Safe local skill inventory and synchronization"
status: accepted
liveness: auto
tags: [skills, agents, omarchy]
---

## Problem

Omarchy users keep reusable Skills across Claude Code, Codex, Grok, and
Octoscode, but each agent discovers and mutates Skills through different
directories or APIs. Copying the same Skill into several homes causes drift,
while editing bundled files or agent profile state directly can break upgrades,
isolation, and credentials.

## Requirements

[REQ-005-INVENTORY] The manager MUST inventory Claude Code, Codex, Grok, and Octoscode Skill surfaces and report agent, canonical name, source class, path or native identifier, activation state, health, byte size, and duplicate relationships as deterministic JSON.

[REQ-005-PRIVACY] Inventory MUST read only directory metadata, bounded `SKILL.md` frontmatter, and documented native inventory output; it MUST NOT read Skill instruction bodies into reports, agent sessions, prompts, logs, credentials, authentication files, or unrelated project contents.

[REQ-005-SHARED] User-managed portable Skills MUST use `~/.agents/skills` as the canonical source; Codex and Claude Code MUST use owned links, Grok MUST use its native `.agents/skills` discovery, and Octoscode MUST use its native profile Skill command or AppUI boundary.

[REQ-005-OWNERSHIP] The manager MUST treat vendor-bundled, system-owned, foreign, and pre-existing unmanaged Skills as read-only and MUST remove only links or receipts whose exact identity was previously recorded by omarchy-rs.

[REQ-005-PLAN] Cross-agent synchronization and cancellation MUST require a persisted exact plan plus confirmation token, revalidate source and destination identities before mutation, and report partial failure per agent without rolling a successful native Agent mutation back through direct filesystem edits.

[REQ-005-OCTOS] Octoscode integration MUST NOT modify `~/.octos/instances`, profile JSON, databases, or session state directly; an unavailable executable, profile, or Skill capability MUST be reported as unavailable rather than inferred as synchronized.

[REQ-005-PLUGIN] A user-owned Omarchy panel MUST consume only the Rust JSON contract, group Skills into Claude Code, Codex, Grok, and Octoscode tabs, apply plans only to the selected Agent, show the shared highlighted Rust backend badge, and remain installable and removable without modifying `/usr/share/omarchy`.

## Scenarios

Rule: REQ-005-INVENTORY
Scenario: Four-agent inventory is normalized
  Given synthetic Claude Code, Codex, Grok, and Octoscode Skill surfaces
  When the manager scans them
  Then JSON reports normalized records and duplicate relationships for all four Agents

Rule: REQ-005-PRIVACY
Scenario: Inventory excludes instruction bodies and private state
  Given synthetic Skill frontmatter plus prompt, credential, log, and session sentinels
  When inventory JSON is produced
  Then it contains Skill metadata but none of the sentinel contents

Rule: REQ-005-SHARED
Scenario: Shared Skill synchronizes through native Agent boundaries
  Given one valid portable Skill under the synthetic shared root
  When a confirmed synchronization plan targets all four Agents
  Then Claude Code and Codex receive owned links, Grok reports native shared discovery, and Octoscode receives one native install request

Rule: REQ-005-OWNERSHIP
Scenario: Foreign and bundled Skills remain unchanged
  Given bundled, system-linked, and unmanaged destination Skills
  When synchronization or cancellation is requested
  Then the operation refuses conflicting destinations and preserves every foreign byte

Rule: REQ-005-PLAN
Scenario: Stale synchronization plan is rejected
  Given a persisted synchronization plan whose source or destination changes
  When apply receives the matching confirmation token
  Then the changed target remains untouched and the per-Agent result reports an identity rejection

Rule: REQ-005-OCTOS
Scenario: Missing Octos capability is explicit
  Given no usable Octos executable or profile capability
  When inventory or synchronization runs
  Then Octoscode is reported unavailable and no Octos state path is modified directly

Rule: REQ-005-PLUGIN
Scenario: Panel uses the Rust manager contract
  Given the embedded Omarchy Skill Manager panel
  When its process commands are inspected
  Then inventory, plan, and apply invoke `omarchy-rs skills` without a shell or privileged command

Scenario: Panel groups Skills by selected Agent
  Given normalized Skill activation records for all four Agents
  When the user changes the selected Agent tab
  Then only that Agent's Skills and activation states are shown and plans target only that Agent

## Dependencies

- REQ-001
- REQ-003

## Source Trace

- User confirmation on 2026-08-20: manage Claude Code, Codex, Grok, and
  Octoscode from the Omarchy layer.
- User confirmation on 2026-08-20: group the panel by Agent tabs and use the
  same highlighted Rust badge as the Agent Usage panel.
- Omarchy shared layout on this host: `~/.agents/skills`, with owned links from
  Claude Code and Codex and native discovery documented by Grok.
- Octos source: `Config::plugin_dirs_from_project`, profile Skill AppUI methods,
  and `octos skills --profile` establish native discovery and mutation
  boundaries without direct instance editing.

## Open Questions

None.

## Next

Compile this requirement into the local Skill manager engine, CLI, and Omarchy
panel task contract.
