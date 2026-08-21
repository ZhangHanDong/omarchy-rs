spec: task
name: "Network Inspector Plugin"
inherits: project
tags: [network, sniffnet, plugin, agents, privacy]
satisfies: [REQ-008]
depends: [task-learn-books-agent-translation]
estimate: 2d
---

<!-- lint-ack: verification-metadata-suggestion — External process tests use isolated executable stubs and synthetic roots with no real network or Agent data. -->

## Intent

Add a user-owned Omarchy Network Inspector panel backed by `omarchy-rs`.
Expose content-free local health, launch or focus the independent Sniffnet GUI,
and let a selected Agent open an interactive terminal with the bounded snapshot
already supplied as its first network-diagnosis question.

## Decisions

- Read an injected `/proc/net/route`, `/sys/class/net`, and resolver file; report only interface kind, carrier, byte counters, default-route presence, and DNS configuration presence.
- Detect Sniffnet by direct executable resolution, running process name, and Linux capture capability text; never inspect packets, PCAPs, connections, remote hosts, URLs, SSIDs, or address values.
- Persist a SHA-256-bound `DiagnosisPlan` under `$XDG_STATE_HOME/omarchy-rs/network/plans` with a separate confirmation token.
- Launch Codex, Claude Code, or Grok in an independent Omarchy terminal through a direct launcher argv; keep Octoscode unavailable until it has an equivalent public interactive interface.
- Start the selected Agent in its normal interactive mode with the exact content-free snapshot as the initial question, so diagnosis and follow-up stay inside the Agent terminal instead of the panel.
- Keep the plan confirmation check in the internal terminal-session entry point; reject a changed snapshot before replacing that process with the selected Agent.
- Open Sniffnet only from the explicit `network open` command: focus a running window through direct `hyprctl` argv or spawn one detached process.
- Match a running Sniffnet PID to its Hyprland client and focus the exact window address; persist only a typed content-free operation outcome so focus failures can be included in a later diagnosis plan.
- Install `omarchy-rs.network-inspector` only below `$XDG_CONFIG_HOME/omarchy/plugins`, track exact hashes, and refuse foreign or modified plugin files.
- Refuse plugin installation without Sniffnet and collapse the bar widget whenever the status snapshot reports Sniffnet unavailable.
- Optimistically switch Open to Focus in the click handler, roll back through status on failure, show terminal-launch status without embedding Agent output, and place the shared Rust badge in `PanelHero.trailingControl`.

## Boundaries

### Allowed Changes
- src/lib.rs
- crates/omarchy-cli/src/main.rs
- crates/omarchy-network/**
- plugins/omarchy-rs.network-inspector/**
- plugins/common/RustBadge.qml
- docs/components/network-inspector.md
- README.md
- knowledge/requirements/req-008-guarded-network-inspector-and-agent-diagnosis.md
- specs/task-network-inspector-plugin.spec.md

### Forbidden
- Do not modify sibling Omarchy, Sniffnet, `/usr/share/omarchy`, `/usr/bin`, NetworkManager, firewall, routes, DNS, VPN, kernel settings, or privilege configuration.
- Do not add packet capture, a background daemon, network probes, shell execution, sudo, pkexec, or automatic repair.
- Do not read real Agent logs, credentials, prompts, browser state, PCAP files, connection histories, SSIDs, remote hosts, or packet content in tests.

## Completion Criteria

### Rule: content-free-status — Gather only bounded local health

Scenario: Synthetic roots produce the expected status record
  Test:
    Package: omarchy-rs
    Filter: network_status_uses_synthetic_content_free_roots
  Given isolated route, interface, resolver, process, and executable fixtures
  When the Rust status collector runs
  Then its JSON reports route, link, counters, DNS presence, and Sniffnet readiness without address or resolver values

Scenario: Malformed and sensitive inputs fail closed
  Test:
    Package: omarchy-rs
    Filter: network_status_excludes_sensitive_and_malformed_values
  Given malformed records containing synthetic SSID, URL, credential, address, and payload markers
  When status and plan artifacts are serialized
  Then zero marker bytes occur and malformed numbers become unavailable rather than panicking

### Rule: explicit-sniffnet-open — Launch no GUI without direct intent

Scenario: Open chooses one launch or focus action
  Test:
    Package: omarchy-rs
    Filter: network_open_launches_or_focuses_exactly_once
  Given isolated Sniffnet and Hyprland executable stubs for stopped and running states
  When `network open` executes
  Then each request records exactly one direct argv action without a shell

Scenario: Missing Sniffnet leaves the system unchanged
  Test:
    Package: omarchy-rs
    Filter: network_open_missing_sniffnet_fails_without_side_effects
  Given an isolated PATH with no Sniffnet executable
  When `network open` executes
  Then it returns unavailable and creates no process receipt

Scenario: Focus uses the exact Hyprland client address
  Test:
    Package: omarchy-rs
    Filter: network_focus_matches_hyprland_client_by_pid
  Given isolated process and Hyprland client fixtures
  When a running Sniffnet is opened
  Then the client address belonging to its PID is focused without a class-name assumption

Scenario: Focus failure enters the diagnosis snapshot
  Test:
    Package: omarchy-rs
    Filter: network_focus_failure_enters_diagnosis_snapshot
  Given a running Sniffnet with no matching Hyprland client
  When focus fails and a diagnosis plan is created
  Then the plan contains only the typed focus issue and no Hyprland output

### Rule: interactive-agent-terminal — Continue diagnosis in the selected Agent

Scenario: Supported Agents receive the exact snapshot in an interactive terminal
  Test:
    Package: omarchy-rs
    Filter: network_agent_terminal_launch_is_direct_and_contextual
  Given isolated terminal-launcher and Codex, Claude, and Grok executable stubs
  When an Agent terminal is requested and its confirmed session starts
  Then the launcher receives one bounded internal command and the interactive Agent receives the exact content-free diagnostic context

Scenario: Wrong token or changed snapshot rejects diagnosis
  Test:
    Package: omarchy-rs
    Filter: network_diagnosis_rejects_wrong_or_stale_plan
  Given a persisted plan and an isolated Agent receipt path
  When terminal-session confirmation is wrong or the synthetic network snapshot changes
  Then the interactive Agent is not executed

Scenario: Octos and unavailable terminal launchers fail without an Agent process
  Test:
    Package: omarchy-rs
    Filter: network_agent_terminal_rejects_unsupported_or_missing_tools
  Given Octos selection or an isolated environment without the configured terminal launcher
  When an Agent terminal is requested
  Then a typed unavailable error is returned and no Agent process starts

### Rule: user-owned-network-plugin — Integrate without package mutation

Scenario: Plugin install and uninstall preserve ownership
  Test:
    Package: omarchy-rs
    Filter: network_plugin_install_is_user_owned
  Given an isolated user configuration root
  When plugin install and uninstall execute
  Then manifest, panel, Rust badge, and ownership marker are added and removed without touching package paths

Scenario: Plugin invokes only the guarded Rust interface
  Test:
    Package: omarchy-rs
    Filter: network_plugin_uses_guarded_json_commands
  Given the bundled QML source
  When its process commands are inspected
  Then it invokes only `omarchy-rs network` status, open, and agent-terminal operations without a shell

Scenario: Missing Sniffnet cannot leave an enabled widget
  Test:
    Package: omarchy-rs
    Filter: network_plugin_requires_sniffnet
  Given an isolated user environment without a Sniffnet executable
  When plugin installation is requested
  Then installation is refused and no plugin directory is created

## Out of Scope

- Embedding Sniffnet's Iced window inside Quickshell.
- Reading Sniffnet capture data or displaying packet-level charts in the panel.
- Applying Agent-proposed fixes or privileged network mutations.
- Replacing `omarchy-network-status`.
