---
kind: requirement
id: REQ-008
title: "Guarded Network Inspector and Agent diagnosis"
status: accepted
liveness: auto
tags: [network, sniffnet, plugin, agents, privacy]
---

## Problem

Omarchy users can inspect connection state and launch separate networking
tools, but they lack one user-owned panel that explains whether Sniffnet is
ready and lets an Agent reason over a bounded diagnostic snapshot. Network
diagnostics can expose browsing activity and credentials, so integration must
remain useful without collecting packets or private connection history.

## Requirements

[REQ-008-STATUS] The system MUST produce a deterministic local network snapshot from explicitly configured proc, sysfs, and resolver roots without packet capture or network probes.

[REQ-008-PRIVACY] The snapshot and every persisted plan or result MUST exclude packet payloads, PCAP data, remote hosts, URLs, SSIDs, addresses, DNS server values, credentials, cookies, prompts, and private Agent state.

[REQ-008-SNIFFNET] The system MUST report Sniffnet installation, running, and capture-permission readiness and MUST launch or focus it only after an explicit user action.

[REQ-008-FOCUS] A running Sniffnet MUST be focused through the exact Hyprland client address associated with its process ID; a failed open or focus MUST enter the content-free diagnostic snapshot as a typed issue.

[REQ-008-PLAN] Agent diagnosis MUST use a persisted exact snapshot plan and confirmation token and MUST reject changed, missing, or mismatched plans before starting the interactive Agent.

[REQ-008-AGENTS] Codex, Claude Code, and Grok MUST open in an independent Omarchy terminal with the content-free snapshot supplied as the initial interactive question; unsupported Octoscode MUST be reported unavailable.

[REQ-008-ADVICE] The panel MUST NOT capture, render, persist, or execute Agent answers; diagnosis, follow-up, permissions, and proposed actions MUST remain visible and controllable in the selected Agent terminal.

[REQ-008-PLUGIN] The Omarchy plugin MUST install only below the user configuration root with ownership verification, MUST remain hidden when Sniffnet is unavailable, and MUST invoke only `omarchy-rs network` JSON commands without a shell.

## Scenarios

Rule: REQ-008-STATUS
Scenario: Synthetic local state produces a content-free snapshot
  Given isolated proc, sysfs, resolver, and executable roots
  When Network Inspector gathers status
  Then it reports interface, route, carrier, counters, DNS readiness, and Sniffnet readiness without private values

Rule: REQ-008-PRIVACY
Scenario: Sensitive network markers never enter artifacts
  Given synthetic addresses, SSIDs, URLs, credentials, payloads, and remote hosts
  When snapshot, plan, and result JSON are serialized
  Then zero sensitive markers occur in those artifacts

Rule: REQ-008-SNIFFNET
Scenario: Explicit open launches or focuses Sniffnet
  Given synthetic Sniffnet and focus executables
  When open is requested for stopped and running states
  Then exactly one direct launch or focus request is recorded

Rule: REQ-008-FOCUS
Scenario: Focus and focus failure are deterministic
  Given synthetic process and Hyprland client records
  When Sniffnet focus succeeds or fails
  Then the exact client address is focused or a typed content-free issue becomes available to Agent diagnosis

Rule: REQ-008-PLAN
Scenario: Diagnosis requires exact confirmation
  Given a persisted diagnosis plan
  When its token is wrong or its snapshot identity changes
  Then no Agent starts and no result is written

Rule: REQ-008-AGENTS
Scenario: Supported Agents receive a bounded direct invocation
  Given synthetic Codex, Claude, Grok, and Octos executables
  When the Agent terminal is requested
  Then one direct terminal-launcher argv starts the selected interactive Agent with the bounded context and Octos reports unavailable

Rule: REQ-008-ADVICE
Scenario: Agent advice remains in the Agent terminal
  Given a confirmed interactive diagnosis session
  When the Agent answers or asks for permission
  Then the panel stores no answer and executes no proposed action

Rule: REQ-008-PLUGIN
Scenario: User plugin installation is owned and reversible
  Given an isolated user configuration root
  When the Network Inspector plugin is installed and uninstalled
  Then only owned plugin files change and foreign files are refused

## Dependencies

- REQ-001
- REQ-006

## Source Trace

- User-approved design on 2026-08-21: integrate Sniffnet through an Omarchy UI and add guarded Agent diagnosis.
- Sniffnet official repository inspected on 2026-08-21: Sniffnet is an independent Rust/Iced packet-analysis GUI, not an embeddable QML component or status library.
- Existing Omarchy terminal launcher and omarchy-rs plan confirmation establish the reusable interactive safety model.

## Open Questions

None.

## Next

Implement the bounded Network Inspector CLI, user-owned plugin, Sniffnet launcher, Agent diagnosis, synthetic tests, and user guide.
