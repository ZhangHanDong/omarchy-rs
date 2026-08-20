# Local Skill Manager

The Local Skill Manager is an Omarchy bar plugin backed by the Rust
`omarchy-rs skills` engine. It manages only the user layer and does not patch
Omarchy or any supported Agent.

## Quick start

```bash
cargo install omarchy-rs
omarchy-rs skills install-plugin
omarchy plugin enable omarchy-rs.skills
```

Click the orange **S** in the Omarchy bar. The highlighted Rust badge confirms
that inventory and plan operations are being handled by the Rust component.
Choose an Agent tab, select a Skill, review the requested operation, and then
confirm it. Opening or refreshing the panel is read-only.

## Model

`~/.agents/skills` is the canonical portable Skill directory. Each child
directory is expected to contain a `SKILL.md` with bounded YAML frontmatter.
The scanner reads only that frontmatter; it does not serialize the instruction
body or inspect credentials, prompts, logs, sessions, databases, or profiles.

The four adapters behave as follows:

- Claude Code: manager-owned link in `~/.claude/skills`.
- Codex: manager-owned link in `~/.codex/skills`.
- Grok: native discovery of `~/.agents/skills`; no duplicate is created.
- Octoscode: its public `octos skills --profile ...` command. The manager never
  writes Octos instance directories or private state directly.

If Octoscode is absent, inventory and the other adapters continue working and
the report marks Octoscode unavailable.

## Add a portable Skill

Create one directory per Skill under the canonical user store:

```text
~/.agents/skills/
└── my-skill/
    └── SKILL.md
```

The file needs bounded frontmatter containing a portable name. A description
is recommended because future detail views and CLI consumers can display it:

```markdown
---
name: my-skill
description: Explain when and how this Skill should be used.
---

Skill instructions go here.
```

Names may contain ASCII letters, digits, and hyphens and must be at most 64
characters. After adding a Skill, choose **Refresh** or run
`omarchy-rs skills scan --json`. The manager reads the frontmatter but never
includes the instruction body in its report.

## Install the Omarchy plugin

Install `omarchy-rs` first, then install and enable its user-owned panel:

```bash
cargo install omarchy-rs
omarchy-rs skills install-plugin
omarchy plugin enable omarchy-rs.skills
```

The plugin is installed below `~/.config/omarchy/plugins/omarchy-rs.skills`.
It does not modify `/usr/share/omarchy`. Remove only files installed by this
version with:

```bash
omarchy plugin disable omarchy-rs.skills
omarchy-rs skills uninstall-plugin
```

## Use the CLI

Inventory is read-only:

```bash
omarchy-rs skills scan --json
```

Mutations use two explicit phases. First create and review a persisted plan:

```bash
omarchy-rs skills plan --skill my-skill --operation sync \
  --agent claude --agent codex --agent grok --agent octoscode --json
```

Then pass the returned `id` and `confirmationToken` unchanged:

```bash
omarchy-rs skills apply --plan PLAN_ID --confirm CONFIRMATION_TOKEN --json
```

Use `--operation cancel` to remove activation. Cancellation removes only links
or Octos installations recorded as manager-owned. A regular file, directory,
foreign link, changed source, stale plan, or wrong token is refused and
reported per Agent.

The Octos profile defaults to `octos`. Override its public CLI path or profile
for all phases when needed:

```bash
export OMARCHY_RS_OCTOS="$HOME/.octos/bin/octos"
export OMARCHY_RS_OCTOS_PROFILE="octos"
```

## Panel workflow

Open the Skill icon and switch between the **Claude**, **Codex**, **Grok**, and
**Octos** tabs. Each tab shows only that Agent's Skills and activation state.
Select a Skill and choose **Review sync** or **Review cancel**; the generated
plan targets only the selected Agent and appears before **Confirm** becomes
available. Refresh after applying to inspect the normalized state.

The selected Tab controls the mutation target. For example, **Review sync** in
the Codex tab creates a Codex-only plan; it does not silently activate the
Skill for Claude, Grok, or Octos. Switch tabs and repeat when different Agents
should receive the same portable Skill.

### Status reference

| Panel status | Meaning |
| --- | --- |
| `active` | The selected Agent currently discovers the Skill. |
| `available` | A portable shared Skill can be synchronized to this Agent. |
| `not installed` | The record has no activation surface for this Agent. |
| `conflict` | A foreign file, directory, or link already owns the destination. |
| `unavailable` | The Agent executable or required native capability is absent. |
| `Needs attention` | `SKILL.md` is missing, malformed, or exceeds the frontmatter bound. |

Agent-native and vendor-bundled Skills appear for visibility but remain
read-only. **Review sync** is enabled only for a portable Skill whose canonical
source is under `~/.agents/skills`. Cancellation removes only an activation
previously recorded as owned by omarchy-rs.

The highlighted Rust badge uses the same packaged `RustBadge.qml` component as
the Workspace Cleaner. Each plugin receives a local copy from one source in
the Rust crate, avoiding fragile cross-plugin runtime imports.

The QML panel is presentation only. Every scan, plan, validation, and adapter
operation is performed by the Rust CLI with direct argument arrays and no
shell or privilege escalation.

## Troubleshooting

### The bar icon is missing

Rescan the user plugin directory, enable the widget, and restart the shell:

```bash
omarchy-shell shell rescanPlugins
omarchy plugin enable omarchy-rs.skills
omarchy restart shell
```

### The panel prints CLI usage text

The panel and CLI are from different releases. Upgrade the crate, reinstall
the embedded plugin, and restart the shell:

```bash
cargo install omarchy-rs --force
omarchy-rs skills install-plugin
omarchy restart shell
```

### Synchronization reports `foreign-destination`

The destination already exists and is not owned by omarchy-rs. The manager
will not overwrite it. Inspect the reported path and decide manually whether
to keep, rename, or migrate that existing Skill; do not delete it merely to
silence the warning.

### A plan is rejected after review

Plans intentionally fail closed if their token is wrong or if the Skill or
destination changed after planning. Refresh the panel and create a new plan.

### Octos is unavailable

Confirm that the executable and profile are correct, then reopen the panel:

```bash
export OMARCHY_RS_OCTOS="$HOME/.octos/bin/octos"
export OMARCHY_RS_OCTOS_PROFILE="octos"
omarchy-rs skills scan --json
```

## Current scope

Version 0.1.3 provides local inventory, per-Agent tabs, health reporting, and
guarded synchronization/cancellation. It does not edit Skill instruction
bodies, search a remote marketplace, execute third-party installers, or scan
all repositories under `~/Work` for project-local Skills.
