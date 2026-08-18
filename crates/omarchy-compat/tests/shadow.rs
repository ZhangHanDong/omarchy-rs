#[cfg(unix)]
mod unix {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};

    use tempfile::TempDir;

    #[test]
    fn shadow_mode_preserves_upstream_output() {
        let isolated = TempDir::new().unwrap();
        let home = isolated.path().join("home");
        let upstream = isolated.path().join("upstream-codex");
        fs::create_dir_all(home.join(".codex/sessions")).unwrap();
        let record = r#"{"schemaVersion":1,"id":"codex","name":"Codex","ready":true,"limits":[],"hasLocalStats":true,"todayPrompts":99,"todaySessions":0,"todayTotalTokens":0,"todayTokensByModel":{},"recentDays":[],"totalPrompts":99,"totalSessions":0,"activeDays":0,"activeDates":[],"modelUsage":{}}"#;
        fs::write(&upstream, format!("#!/bin/sh\nprintf '%s\\n' '{record}'\n")).unwrap();
        let mut permissions = fs::metadata(&upstream).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&upstream, permissions).unwrap();

        let output = Command::new(env!("CARGO_BIN_EXE_omarchy-agent-usage-codex-shadow"))
            .env_clear()
            .env("HOME", &home)
            .env("CODEX_HOME", home.join(".codex"))
            .env("OMARCHY_RS_CODEX_UPSTREAM", &upstream)
            .output()
            .unwrap();

        assert!(output.status.success());
        let output_record: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let mut expected: serde_json::Value = serde_json::from_str(record).unwrap();
        expected["collectorBackend"] = "python".into();
        assert_eq!(output_record, expected);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("omarchy-rs-shadow"));
        assert!(stderr.contains("todayPrompts"));
        assert!(!stderr.contains("99"));
    }
}
