# Learn Books and Agent Translation

`omarchy-rs learn` extends Omarchy's existing **Learn** menu entirely from the
user layer. It does not edit `/usr/share/omarchy` and does not replace the
original Omarchy, Hyprland, Arch, Neovim, or Bash links.

## Install the Learn integration

Install the current CLI and synchronize the owned menu block:

```bash
cargo install omarchy-rs
omarchy-rs learn sync-menu
```

Upgrade an existing installation before using features from a newer release:

```bash
cargo install --force omarchy-rs
omarchy-rs install
omrs learn sync-menu
```

`omarchy-rs install` refreshes the user-owned executable overlay and creates
the short `omrs` alias. Both command names run the same binary. It does not
replace files under `/usr/bin` or `/usr/share/omarchy`.

Omarchy watches `~/.config/omarchy/extensions/omarchy-menu.jsonc`, so the new
entries normally appear immediately. If necessary:

```bash
omarchy-shell shell rescanPlugins
```

The synchronization preserves comments, whitespace, and foreign entries. It
owns only the lines between these markers:

```text
// BEGIN OMARCHY-RS LEARN (managed; do not edit)
// END OMARCHY-RS LEARN
```

## Add a Book

Only public HTTPS pages are accepted. IDs use lowercase letters, digits, and
interior hyphens:

```bash
omrs learn add \
  --id rust-book \
  --label "The Rust Programming Language" \
  --url "https://doc.rust-lang.org/book/" \
  --description "The official Rust Book"

omrs learn sync-menu
```

For an mdBook deployed with GitHub Pages, register the published site rather
than the GitHub repository URL. For example:

```bash
omrs learn add \
  --id pi-book \
  --label "Pi Book" \
  --url "https://zhanghandong.github.io/pi-book/" \
  --description "pi 的设计艺术：Coding Agent 架构决策"

omrs learn sync-menu
```

The published URL supplies rendered chapter text. A repository URL would
instead supply GitHub's repository page and navigation.

The Book appears directly below **Learn**. User Books are stored in:

```text
~/.config/omarchy-rs/learn/books.json
```

List the built-in and user catalogs:

```bash
omrs learn books --json
```

Remove only a user Book and refresh the menu:

```bash
omrs learn remove --id rust-book
omrs learn sync-menu
```

## Translate a chapter from the menu

Open **Learn → Agent Translate**, choose Codex, Claude, or Grok, then choose a
Book. A terminal displays the exact Book, source URL, target language, Agent,
and one-MiB source bound. Translation begins only after typing the exact
uppercase confirmation `YES`; `yes`, `Yes`, and an empty line cancel it.

The first version translates exactly the configured page. It does not follow
chapter links or crawl the complete Book. Each successful translation is an
escaped local HTML document below:

```text
~/.cache/omarchy-rs/learn/translations/
```

The document records its original URL, Agent, language, and generation time.
The same source identity, Agent, and language reuse the cache without another
paid Agent request.

The menu currently targets `zh-CN`. Use the CLI when the source is already
Chinese or another target language is required:

```bash
omrs learn translate \
  --book pi-book \
  --agent grok \
  --language en-US
```

This interactive command presents the same confirmation before fetching or
invoking the Agent. Supported language values are passed as BCP 47-style
labels to the selected Agent; the first release does not maintain a fixed
language allow-list.

## Plan and apply from the CLI

For scripting, create a reviewable plan:

```bash
omrs learn plan \
  --book rust-book \
  --agent codex \
  --language zh-CN
```

Then apply with the returned values:

```bash
omrs learn apply \
  --plan PLAN_ID \
  --confirm CONFIRMATION_TOKEN
```

The plan expires after one hour. A wrong token or changed Book URL prevents
fetching and Agent invocation.

## Supported Agents

| Agent | Translation boundary |
| --- | --- |
| Codex | Ephemeral `codex exec`, read-only sandbox, isolated working directory, independent web search disabled. |
| Claude Code | Print mode, no session persistence, slash commands and tools disabled. |
| Grok | Single-turn prompt through stdin, tools and web search disabled, one turn. |
| Octoscode | Unavailable until Octos exposes a stable bounded non-interactive translation command. |

Override executable discovery when needed:

```bash
export OMARCHY_RS_LEARN_CODEX=/path/to/codex
export OMARCHY_RS_LEARN_CLAUDE=/path/to/claude
export OMARCHY_RS_LEARN_GROK=/path/to/grok
```

The manager invokes executables directly. It does not start a shell, inspect
Agent logs/sessions/prompts, or read Agent credentials itself. The Agent still
uses its own normal authentication, and translation may consume paid quota.

## Network and output safety

- Only HTTPS hostnames are accepted; URL credentials, credential-like query
  keys, localhost, IP literals, and private DNS destinations are rejected.
- Mihomo Fake-IP DNS is supported while the local `Mihomo` interface is
  active. Its documented IPv4 and IPv6 fake ranges are accepted only in that
  state; ordinary private, loopback, link-local, and unrelated unique-local
  answers remain rejected.
- Every redirect destination is revalidated and at most three redirects are
  followed.
- Only HTML, plain text, and Markdown are accepted, up to one MiB.
- Script and style blocks are excluded from extracted HTML text.
- Agent output is limited to two MiB and five minutes.
- Agent output is escaped before local HTML rendering; returned script markup
  is never executed as HTML.

## Remove the integration

Remove only the owned menu block:

```bash
omrs learn unsync-menu
```

This preserves the Book registry, translation cache, and all foreign Omarchy
menu entries. Remove user Books individually before deleting those directories
manually if a complete data removal is desired.

## Troubleshooting

### Book is in the registry but not in Learn

Run `omrs learn sync-menu`. The registry and menu are intentionally
separate so adding several Books does not rewrite the live menu repeatedly.

### Menu synchronization reports corrupt markers

The manager found a partial, duplicated, or reversed owned block and refused
to guess. Restore the marker block from version control or remove the complete
block manually only after reviewing the surrounding foreign entries.

### Translation plan reports an unavailable Agent

Run `omrs learn books --json` and inspect `agentAvailability`. Confirm
the executable is on `PATH` or set its `OMARCHY_RS_LEARN_*` override.

### The menu terminal closes without showing a result

Run the same translation directly in an existing terminal so the error stays
visible:

```bash
omrs learn translate \
  --book BOOK_ID \
  --agent grok \
  --language zh-CN
```

Type uppercase `YES`. If successful, the command prints the cache path and
opens the local HTML document. Inspect existing results without reading Agent
private state:

```bash
find ~/.cache/omarchy-rs/learn/translations -maxdepth 1 -type f
```

If Grok rejects its command-line arguments, confirm that `grok --help` lists
`--prompt-file`, `--output-format plain`, `--disable-web-search`, and
`--max-turns`. Upgrade Grok or select Codex/Claude when those stable
single-turn flags are unavailable.

### A public Book is rejected as private while Mihomo is running

Version 0.1.4 and later recognize Mihomo Fake-IP DNS only when the `Mihomo`
network interface exists. Confirm the active interface and resolved answers:

```bash
ip -brief address show Mihomo
getent ahosts BOOK_HOSTNAME
```

Do not bypass the safety check for arbitrary private addresses. Upgrade
`omarchy-rs` if an older release rejects Mihomo's fake IPv4 or IPv6 answer.

### Translation plan becomes stale

The Book changed after review or the one-hour plan expired. Create a new plan;
stale plans deliberately cannot be forced through.

### A whole mdBook was not translated

This is expected. A Book entry identifies one HTTPS page, and translation
never crawls the mdBook sidebar or follows chapter links. Register a specific
published chapter URL when that chapter, rather than the landing page, is the
desired translation source.
