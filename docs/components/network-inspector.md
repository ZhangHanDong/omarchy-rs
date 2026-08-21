# Network Inspector

Network Inspector is a user-owned Omarchy panel backed by the `omarchy-rs`
CLI. It reports content-free local network health, launches or focuses the
independent Sniffnet GUI, and can open Codex, Claude Code, or Grok in a terminal
with a content-free diagnostic snapshot already supplied as the first question.

It is not a packet sniffer. The panel never reads Sniffnet captures, PCAP
files, remote connections, URLs, SSIDs, addresses, DNS server values, browser
state, credentials, prompts, or Agent history.

## Install

Install Sniffnet separately if you want packet-level inspection. Network
Inspector intentionally does not install packages or change capture
permissions.

The plugin is installable and visible only while a `sniffnet` executable is
available. Removing Sniffnet makes the widget collapse out of the bar on its
next status refresh; the read-only `omarchy-rs network status` CLI remains
available for troubleshooting.

```bash
omarchy pkg add sniffnet
cargo install omarchy-rs --force
omarchy-rs install
omarchy-rs network install-plugin
omarchy bar put omarchy-rs.network-inspector --section right
```

Saving the plugin below `~/.config/omarchy/plugins` triggers the normal shell
plugin rescan. If it does not appear, run `omarchy-shell shell rescanPlugins`.

## Panel workflow

Open the network icon in the bar. The panel shows whether a default route, link
carrier, resolver configuration, and Sniffnet are available. `Open Sniffnet`
starts the independent Iced window; if a Sniffnet process is already running,
the action matches that process ID to the exact Hyprland client address and
focuses it. A failed launch or focus is recorded as a content-free issue (for
example `sniffnet-window-unavailable`) so the next Agent diagnosis can explain
the failure without receiving window titles or compositor output.

For interactive Agent diagnosis:

1. Select Codex, Claude, or Grok.
2. Choose **Ask in codex**, **Ask in claude**, or **Ask in grok**.
3. Continue the conversation in the newly opened terminal. The Agent receives
   the current bounded snapshot and is asked to explain evidence, request any
   additional safe checks, and obtain approval before making changes.

The panel does not wait for, capture, render, or persist the answer. Markdown,
follow-up questions, tool approvals, and session history are handled by the
Agent's own interactive terminal UI. This removes the panel timeout and keeps
the entire troubleshooting conversation visible and controllable in one place.

The plan is bound to the stable health identity. If the route, interface,
carrier, DNS readiness, Sniffnet installation, or capture readiness changes,
confirmation fails and a new plan is required. Traffic counters are shown but
excluded from that identity so normal traffic does not invalidate consent.

## CLI

```bash
omarchy-rs network status --json
omarchy-rs network open --json
omarchy-rs network agent-terminal --agent codex --json
omarchy-rs network uninstall-plugin
```

`status` reads local procfs, sysfs, and resolver configuration only. It does
not ping a host or make a network request. `agent-terminal` persists an exact
snapshot plan below `$XDG_STATE_HOME/omarchy-rs/network/plans`, launches an
Omarchy terminal, rechecks that plan, and then replaces the terminal process
with the selected interactive Agent. The internal plan token is not an Agent
credential.

Octoscode is unavailable for this feature until it exposes a supported public
interactive terminal interface.

## Capture permissions

The panel reports whether the Sniffnet executable advertises both
`cap_net_raw` and `cap_net_admin`. It never changes capabilities or invokes
`sudo`/`pkexec`. Follow Sniffnet's own installation documentation when capture
permission is missing, and review any privileged command yourself.

## Uninstall

```bash
omarchy plugin disable omarchy-rs.network-inspector
omarchy-rs network uninstall-plugin
```

Uninstall refuses modified or foreign plugin directories and never touches
the Omarchy package under `/usr/share/omarchy`.
