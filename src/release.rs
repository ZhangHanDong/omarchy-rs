#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, path::Path};

    const MANIFEST: &str = include_str!("../Cargo.toml");
    const LOCK: &str = include_str!("../Cargo.lock");
    const EXPECTED: [&str; 6] = [
        "omarchy-agent-usage-claude-shadow",
        "omarchy-agent-usage-codex-shadow",
        "omarchy-agent-usage-grok",
        "omarchy-agent-usage-octoscode-shadow",
        "omarchy-agent-usage-update",
        "omarchy-rs",
    ];

    #[test]
    fn release_manifest_has_single_public_package() {
        assert!(MANIFEST.contains("name = \"omarchy-rs\""));
        let version = format!("version = \"{}\"", env!("CARGO_PKG_VERSION"));
        assert!(MANIFEST.contains(&version));
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for old in [
            "crates/omarchy-agents/Cargo.toml",
            "crates/omarchy-compat/Cargo.toml",
            "crates/omarchy-cli/Cargo.toml",
        ] {
            assert!(
                !root.join(old).exists(),
                "nested public package remains: {old}"
            );
        }
    }

    #[test]
    fn release_manifest_has_no_git_dependencies() {
        assert!(!MANIFEST.contains("git ="));
        assert!(!LOCK.contains("source = \"git+"));
    }

    #[test]
    fn release_binary_set_is_complete() {
        let actual = MANIFEST
            .lines()
            .filter_map(|line| line.strip_prefix("name = \"")?.strip_suffix('\"'))
            .filter(|name| name == &"omarchy-rs" || name.starts_with("omarchy-agent-usage-"))
            .collect::<BTreeSet<_>>();
        let expected = EXPECTED.into_iter().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
    }
}
