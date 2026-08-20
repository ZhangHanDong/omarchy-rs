spec: task
name: "Provider Quota Capability Status"
inherits: project
tags: [agent-usage, quota, grok, octoscode]
satisfies: [REQ-007]
depends: [task-grok-usage-collector, task-octoscode-usage-parity]
estimate: 0.5d
---

## Intent

Make absent Grok and Octoscode quota windows explicit in the Agent Usage panel.
Keep local Rust token aggregation while directing personal Grok users to its
official Usage page and identifying Octoscode provider quotas as unavailable.

## Decisions

- Keep `limits` empty unless an authoritative provider interface supplies a percentage and window.
- Set Grok `usageStatusText` to `Weekly quota: Grok Settings → Usage` and explain that personal Grok exposes no supported CLI quota API.
- Set Octoscode `usageStatusText` to `Provider quotas unavailable` and explain that its ledger contains usage totals, not quota snapshots.
- Do not add xAI Management API support because it is a Business team billing interface rather than personal Grok subscription usage.
- Do not read browser state, credentials, prompts, responses, or private provider configuration.

## Boundaries

### Allowed Changes
- crates/omarchy-agents/src/grok.rs
- crates/omarchy-agents/src/octoscode.rs
- docs/components/agent-usage.md
- knowledge/requirements/req-007-authoritative-provider-quota-capabilities.md
- specs/task-provider-quota-capability-status.spec.md

### Forbidden
- Do not modify the Grok or Octoscode installations.
- Do not add network requests, Management Key support, credential discovery, or browser scraping.
- Do not infer quota percentages or reset times from local token usage.

## Completion Criteria

### Rule: personal-grok-quota-route — Personal Grok exposes its official quota route

Scenario: Grok local usage keeps truthful quota status
  Test:
    Package: omarchy-rs
    Filter: grok_personal_quota_points_to_settings
  Given a synthetic Grok home containing completed local turns
  When the Rust collector creates its record
  Then `limits` is empty and its status points to `Grok Settings → Usage`

Scenario: Empty Grok state does not fabricate a window
  Test:
    Package: omarchy-rs
    Filter: grok_empty_home_has_no_local_stats
  Given an empty synthetic Grok home
  When the Rust collector creates its record
  Then it reports no local totals and no Session, 5-hour, Weekly, or Monthly percentage

### Rule: octos-provider-quota-boundary — Octoscode separates usage from quotas

Scenario: Octoscode model totals do not become quota percentages
  Test:
    Package: omarchy-rs
    Filter: octoscode_provider_quotas_are_explicitly_unavailable
  Given a synthetic Octoscode ledger with model token totals
  When the Rust collector creates its record
  Then `limits` is empty and its status states `Provider quotas unavailable`

Scenario: Malformed Octoscode input cannot invent limits
  Test:
    Package: omarchy-rs
    Filter: octoscode_malformed_and_repeated_turns
  Given malformed and repeated synthetic Octoscode events
  When the Rust collector scans the ledger
  Then malformed events are skipped and no quota percentage is emitted

## Out of Scope

- Automating the personal Grok website.
- xAI Business Management API billing data.
- Adding a new quota protocol to Octoscode or its upstream providers.
