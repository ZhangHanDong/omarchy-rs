# User overlay deployment

`omarchy-rs` installs Agent Usage acceleration entirely under the current
user's XDG data and config homes. It does not require sudo and does not write
to `/usr/bin`, `/usr/share/omarchy`, or `/etc`.

Build the release siblings and install them:

```bash
cargo build --release --workspace
target/release/omarchy-rs doctor
target/release/omarchy-rs install
```

The default layout is:

```text
~/.local/share/omarchy-rs/
├── bin/
│   ├── omarchy-rs -> ../libexec/omarchy-rs
│   └── omarchy-agent-usage-update -> ../libexec/omarchy-agent-usage-update
└── libexec/
    ├── omarchy-rs
    ├── omarchy-agent-usage-update
    ├── omarchy-agent-usage-grok
    └── omarchy-agent-usage-*-shadow

~/.config/omarchy-rs/
├── install.json
└── activation.json
```

Add `~/.local/share/omarchy-rs/bin` before `/usr/share/omarchy/bin` in the
graphical session PATH. Activation refuses to proceed when this precedence is
not observable; it never creates a shim that cannot take effect.

```bash
omarchy-rs activate agent-usage
omarchy-rs status --json
```

The activation record enables Codex, Claude Code, Octoscode, and Grok
independently. The updater supplies each Rust shadow's canary environment
itself, so activation does not depend on shell-specific exported mode
variables. The first three shadows still check the pinned Python collector
fingerprint and fall back to Python after upstream drift or candidate
rejection. Grok is a native-only addition backed by local structured completion
metadata and has no stock Python collector to fall back to.

Rollback is offline and retains installed release files for later activation:

```bash
omarchy-rs rollback agent-usage
```

Rollback removes only the owned updater shim and activation record. Command
resolution then falls through to the official updater. A foreign file at the
shim path is never overwritten or removed.

`doctor --json` and `status --json` report installation, activation, PATH
precedence, resolved updater, and per-provider compatibility. They do not run
collectors or change files.
