spec: task
name: "Learn Books and Guarded Agent Translation"
inherits: project
tags: [learn, books, translation, agents, rust]
satisfies: [REQ-001, REQ-006]
depends: [task-local-skill-manager-plugin]
estimate: 5d
---

<!-- lint-ack: platform-decision-tag — This is intentionally an Omarchy Linux user-layer integration. -->
<!-- lint-ack: output-mode-coverage — JSON is the machine contract; translated HTML is a local presentation artifact. -->
<!-- lint-ack: flag-combination-coverage — Agent/language combinations share one validated plan path and are covered by the adapter matrix. -->

## Intent

Add user-owned Books to Omarchy Learn and translate one configured chapter at
a time through safe public coding-Agent CLI surfaces. Preserve the user's
menu extension and Agent-private state while making every costly external
translation an explicit, reviewable operation.

## Decisions

- Expose `omarchy-rs learn books|add|remove|sync-menu|unsync-menu|plan|apply|open` with deterministic JSON where applicable.
- Store Books at `$XDG_CONFIG_HOME/omarchy-rs/learn/books.json`, plans under `$XDG_STATE_HOME/omarchy-rs/learn/plans`, and escaped HTML under `$XDG_CACHE_HOME/omarchy-rs/learn/translations`.
- Seed a read-only built-in catalog for Omarchy, Hyprland, Arch, Neovim, and Bash translation without replacing their original Learn links.
- Insert only an exact comment-delimited owned block before the final object brace in the user JSONC menu; reject duplicate/corrupt markers and preserve every other byte.
- Accept only HTTPS hostnames without URL credentials or IP literals; follow at most three redirects, revalidate every redirect destination, accept HTML/plain/Markdown, read at most 1 MiB, and use a 20-second network timeout.
- Accept Mihomo Fake-IP DNS ranges only while the local `Mihomo` network interface exists; continue rejecting loopback, private, link-local, and unrelated unique-local addresses.
- Plan before fetching or Agent invocation; apply revalidates registry identity, fetches one page, normalizes bounded text, then invokes exactly one selected Agent.
- Run Codex in ephemeral read-only mode, Claude with no tools and no session persistence, and Grok in single-turn mode with no tools/web search; mark Octos unavailable until it exposes an equivalent stable public boundary.
- Limit Agent stdout to 2 MiB and execution to five minutes; atomically persist escaped HTML with source URL, Agent, language, timestamp, and source identity.
- Tests use only synthetic HOME/XDG roots, fake executables, and a loopback fixture server; production rejects loopback.

## Boundaries

### Allowed Changes
- Cargo.*
- src/**
- crates/omarchy-cli/**
- crates/omarchy-learn/**
- docs/**
- knowledge/requirements/req-006-user-books-and-agent-translation.md
- specs/task-learn-books-agent-translation.spec.md
- README.md

### Forbidden
- Do not modify `../omarchy/**`, `/usr/share/omarchy/**`, `/usr/bin/**`, or system package files.
- Do not crawl an entire Book, execute page scripts, submit forms, or fetch page-linked resources.
- Do not inspect Agent logs, sessions, prompts, credentials, profiles, databases, or unrelated local documents.
- Do not invoke an Agent without an exact confirmed plan or allow its tools, filesystem writes, session persistence, or web fetch.
- Do not overwrite unmarked user menu bytes or follow local/private redirect destinations.
- Do not invoke a shell, `sudo`, `pkexec`, or a package manager from production Rust.

## Completion Criteria

### Rule: book-registry — Maintain deterministic user Books

Scenario: Valid Book add list and remove round-trip
  Test: learn_books_round_trip_deterministically
  Given an isolated XDG config and a valid HTTPS Book
  When add, list, and remove execute
  Then versioned JSON is stable and paths remain below the isolated config root

Scenario: Invalid Book fields are rejected
  Test: learn_books_reject_invalid_ids_and_urls
  Given invalid ids, credentials, HTTP, localhost, and IP-literal URLs
  When a Book add is requested
  Then every request fails and the registry remains byte-identical

### Rule: owned-menu — Extend Learn without owning foreign configuration

Scenario: Menu sync preserves foreign bytes
  Test: learn_menu_sync_preserves_foreign_jsonc
  Given comments, whitespace, and foreign menu entries
  When the owned Book block is synchronized twice
  Then synchronization is idempotent and every byte outside the marker block is unchanged

Scenario: Menu unsync removes only the owned block
  Test: learn_menu_unsync_removes_only_owned_block
  Given a synchronized menu plus later foreign edits
  When menu unsync executes
  Then only the exact owned block is absent

Scenario: Corrupt markers fail closed
  Test: learn_menu_rejects_corrupt_markers
  Given missing, duplicated, or reversed owned markers
  When sync or unsync executes
  Then it returns an error and preserves the complete menu

### Rule: exact-translation-plan — Confirm before external work

Scenario: Translation plan contains metadata but no source body
  Test: learn_plan_excludes_source_and_private_content
  Given a Book and synthetic private sentinels outside the registry
  When a translation plan is serialized
  Then it contains Agent, language, URL, bounds, and cache identity but no fetched or private content

Scenario: Wrong token prevents Agent invocation
  Test: learn_apply_rejects_wrong_confirmation
  Given a persisted plan and a fake Agent executable
  When apply receives another token
  Then the executable is not invoked and no cache file exists

Scenario: Registry drift prevents Agent invocation
  Test: learn_apply_rejects_changed_book
  Given a plan whose Book URL changes before apply
  When apply receives the original token
  Then the executable is not invoked and registry drift is reported

### Rule: safe-source — Fetch one bounded public chapter

Scenario: Redirect destinations are revalidated
  Test: learn_fetch_rejects_private_redirect
  Given an allowed public-looking fixture redirects to loopback
  When apply fetches the chapter through the injectable test transport
  Then it rejects the redirect before returning source bytes

Scenario: Active Mihomo Fake-IP DNS remains usable
  Test: learn_fetch_accepts_active_mihomo_fake_dns_only
  Given the documented Mihomo IPv4 and IPv6 Fake-IP answers
  When resolved addresses are validated with and without an active Mihomo interface
  Then only the active-interface case succeeds and mixed private answers remain rejected

Scenario: Content type and size are bounded
  Test: learn_fetch_rejects_non_text_and_oversized_content
  Given binary and over-one-MiB fixture responses
  When each response is prepared
  Then both fail before Agent invocation

Scenario: Fetch never crawls linked pages
  Test: learn_fetch_reads_exactly_one_document
  Given one HTML page linking a second fixture endpoint
  When source text is prepared
  Then only the configured document endpoint is requested

### Rule: safe-agent-adapters — Invoke only supported bounded interfaces

Scenario: Agent argv disables tools and persistence
  Test: learn_agent_adapters_use_safe_direct_argv
  Given fake Codex, Claude, and Grok executables recording argv
  When each confirmed translation executes
  Then each direct invocation contains its required no-tool, non-persistent, and bounded flags

Scenario: Octos remains explicitly unavailable
  Test: learn_octos_translation_is_unavailable
  Given an installed Octos executable without a supported single-turn boundary
  When translation is planned or applied
  Then capability is unavailable and no Octos path or process is changed

Scenario: Agent failure leaves no completed cache
  Test: learn_agent_failure_is_atomic
  Given a fake Agent exits nonzero after partial output
  When apply executes
  Then the error is bounded and no completed HTML cache exists

### Rule: attributed-cache — Persist safe reusable translations

Scenario: Successful output is escaped and cached
  Test: learn_translation_is_escaped_attributed_and_cached
  Given fake Agent output containing HTML and script markup
  When apply executes twice for the same source identity
  Then local HTML escapes markup, includes source attribution, and invokes the Agent once

Scenario: Output overflow is rejected
  Test: learn_translation_rejects_oversized_agent_output
  Given a fake Agent emits more than two MiB
  When apply executes
  Then the child is terminated and no completed cache exists

## Out of Scope

- Whole-book crawling, recursive link discovery, EPUB/PDF parsing, OCR, or DRM bypass.
- Editing or replacing Omarchy's built-in Learn entries.
- Automatic background translation, implicit spending, translation without confirmation, or remote marketplace discovery.
- A private Octos integration before a safe public non-interactive interface exists.
- Rendering arbitrary Agent HTML without escaping.
