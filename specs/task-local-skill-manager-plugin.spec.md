spec: task
name: "Local Skill Manager for Four Agents"
inherits: project
tags: [skills, agents, omarchy-plugin, rust]
satisfies: [REQ-001, REQ-005]
depends: [task-workspace-cleaner-plugin]
estimate: 5d
---

<!-- lint-ack: platform-decision-tag — This Linux-only Omarchy component intentionally names Agent-specific user paths. -->
<!-- lint-ack: output-mode-coverage — JSON is the sole machine output mode and persisted plans are internal state. -->
<!-- lint-ack: flag-combination-coverage — Agent selectors do not alter rendering and are covered together by the four-Agent scenario. -->

## Intent

Add a Rust inventory and synchronization manager plus an Omarchy bar panel for
local Skills used by Claude Code, Codex, Grok, and Octoscode. Make
`~/.agents/skills` the portable user source while preserving every Agent's
native discovery, profile isolation, bundled content, and update path.

## Decisions

- Expose `omarchy-rs skills scan|plan|apply|install-plugin|uninstall-plugin`; scan, plan, and apply return JSON for QML.
- Model exactly four adapters: `claude`, `codex`, `grok`, and `octoscode`.
- Read portable sources from `$HOME/.agents/skills`; read at most 64 KiB from each `SKILL.md` and emit parsed frontmatter fields, never its instruction body.
- Activate Claude Code and Codex with relative or absolute symlinks recorded in an omarchy-rs ownership receipt; refuse a non-owned destination.
- Treat Grok as active when native `.agents/skills` discovery is available; do not copy a duplicate into `.grok/skills`.
- Invoke the configured Octos executable directly with argv equivalent to `octos skills --profile PROFILE install LOCAL_PATH --force`; never invoke a shell or inspect Octos instance/profile storage.
- Distinguish the Octos native installer from the `octoscode` UI client; when octoscode is installed, expose Codex-native Skills as read-only `backend-visible` records instead of incorrectly hiding them or claiming they were installed into Octos.
- Persist versioned synchronization plans under `$XDG_STATE_HOME/omarchy-rs/skills/plans`; apply requires the plan id and token and revalidates source hashes and destinations.
- Embed and install the QML plugin only at `$XDG_CONFIG_HOME/omarchy/plugins/omarchy-rs.skills`; all four Agent states remain visible when one adapter is unavailable.
- First delivery inventories and synchronizes existing local portable Skills; deleting canonical sources, remote marketplace browsing, and arbitrary project-root scans remain excluded.

## Boundaries

### Allowed Changes
- Cargo.*
- src/**
- crates/omarchy-cli/**
- crates/omarchy-skills/**
- plugins/omarchy-rs.skills/**
- benches/**
- docs/**
- knowledge/requirements/req-005-safe-local-skill-management.md
- specs/task-local-skill-manager-plugin.spec.md
- README.md

### Forbidden
- Do not modify `../omarchy/**`, `../FW/octos/**`, `/usr/share/omarchy/**`, `/usr/bin/**`, or system package files.
- Do not read real Agent logs, sessions, prompts, credentials, authentication files, or full Skill instruction bodies in tests and benchmarks.
- Do not directly edit `~/.octos/instances/**`, Octos profile JSON, databases, or session files.
- Do not overwrite or remove foreign, bundled, system-owned, or unmanaged Skill content.
- Do not invoke a shell, `sudo`, `pkexec`, or a package manager.

## Completion Criteria

### Rule: normalized-inventory — Inventory four distinct Agent surfaces

Scenario: Four Agent adapters normalize Skill metadata
  Test: skills_inventory_normalizes_four_agents
  Given isolated synthetic homes for Claude Code, Codex, Grok, and Octoscode
  When `skills scan --json` runs
  Then every record has agent, name, source class, activation, health, size, and stable identity

Scenario: Duplicate shared Skills are grouped without double-counting
  Test: skills_inventory_groups_shared_duplicates
  Given Claude Code and Codex links plus Grok native discovery refer to one shared Skill
  When inventory totals and duplicates are calculated
  Then the portable Skill bytes are counted once and all three Agent activations reference one canonical identity

### Rule: bounded-private-read — Read metadata without private content

Scenario: Skill body and private Agent state are excluded
  Test: skills_inventory_excludes_bodies_and_private_state
  Given synthetic frontmatter, instruction-body, credential, prompt, log, and session sentinels
  When inventory JSON is serialized
  Then frontmatter name and description appear and no private sentinel appears

Scenario: Oversized or malformed frontmatter is unhealthy
  Test: skills_inventory_rejects_oversized_or_malformed_frontmatter
  Given an oversized `SKILL.md` and malformed frontmatter
  When each Skill is scanned
  Then each record is unhealthy with a bounded reason and scanning reads no trailing body

### Rule: owned-links — Synchronize without overwriting foreign destinations

Scenario: Claude and Codex receive owned links
  Test: skills_apply_creates_owned_claude_and_codex_links
  Given a valid shared Skill and an exact confirmed synchronization plan
  When apply targets Claude Code and Codex
  Then each native Skill root contains a link to the canonical source and the receipt records its identity

Scenario: Foreign destination blocks synchronization
  Test: skills_apply_refuses_foreign_destination
  Given a destination contains an unmanaged file or link
  When a confirmed plan targets that Agent
  Then apply reports conflict and preserves every destination byte

Scenario: Cancellation removes only an owned link
  Test: skills_cancel_removes_only_owned_link
  Given one receipt-owned link and one foreign link
  When a confirmed cancellation plan is applied
  Then only the receipt-owned link is absent

### Rule: native-adapters — Preserve Grok and Octos semantics

Scenario: Grok uses shared discovery without duplicate copy
  Test: skills_grok_adapter_uses_shared_discovery
  Given a portable Skill under `.agents/skills`
  When Grok synchronization is planned and applied
  Then Grok reports active and no `.grok/skills` duplicate is created

Scenario: Octos install uses direct native argv
  Test: skills_octos_adapter_invokes_native_profile_install
  Given a synthetic executable recording argv and an isolated profile name
  When a confirmed plan synchronizes one Skill to Octoscode
  Then exactly one direct install invocation contains the profile, canonical local path, and force flag

Scenario: Unavailable Octos remains non-mutating
  Test: skills_octos_adapter_reports_unavailable
  Given no executable or advertised capability
  When scan and apply run
  Then Octoscode reports unavailable and no Octos state path is created or changed

Scenario: Octoscode shows Codex backend Skills
  Test: skills_octoscode_shows_backend_visible_codex_skills
  Given an installed octoscode client and a Codex-native Agent Spec Skill
  When the local inventory is scanned
  Then the Octos tab includes that Skill as backend-visible without claiming an Octos-native installation

### Rule: exact-plan — Revalidate every synchronization mutation

Scenario: Wrong confirmation token changes nothing
  Test: skills_apply_rejects_wrong_confirmation
  Given a valid persisted synchronization plan
  When apply receives a different token
  Then it returns an error and all Agent destinations remain unchanged

Scenario: Changed source invalidates the plan
  Test: skills_apply_rejects_changed_source
  Given a planned shared Skill changes after planning
  When apply receives the matching token
  Then every Agent mutation is skipped with a source identity reason

### Rule: plugin-lifecycle — Keep the Skill UI user-owned

Scenario: Skill Manager plugin installation is isolated
  Test: skills_plugin_install_is_user_owned
  Given isolated XDG config and the embedded plugin
  When `skills install-plugin` runs
  Then valid plugin files exist only under `omarchy-rs.skills`

Scenario: Skill Manager panel invokes only Rust JSON commands
  Test: skills_plugin_uses_rust_json_commands
  Given the embedded QML panel source
  When process argv is inspected
  Then scan, plan, and apply invoke `omarchy-rs skills` without a shell or privileged command

Scenario: Skill Manager groups by Agent and shares the Rust badge
  Test: skills_panel_groups_agent_tabs_and_highlights_rust
  Given the embedded panel and common Rust badge source
  When the presentation contract is inspected
  Then Claude, Codex, Grok, and Octos tabs filter records and plans target only the selected Agent while the Rust badge is highlighted

## Out of Scope

- Modifying Octos, Grok, Claude Code, Codex, or Omarchy upstream source.
- Recursively scanning every repository under `~/Work` for project-local Skills.
- Deleting canonical directories under `~/.agents/skills`.
- Remote marketplace search, ratings, automatic trust, or executing installer scripts.
- Editing Skill instruction bodies or generating new Skills.
