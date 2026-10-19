//! Offline CLI checks: honest capability report and transport-error
//! passthrough, using `/dev/null` as a stand-in device. No USB hardware.
//!
//! Ported from the earlier Python script; the profile feature was removed
//! from the tool, so those checks are gone too.

use std::fs;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn spawn(envs: &[(&str, &str)], args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_lecoo-gk301"))
        .args(args)
        .env("LC_ALL", "C")
        .envs(envs.iter().copied())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn the CLI");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn offline_cli_checks() {
    let tmp = std::env::temp_dir().join(format!(
        "omalecoo-integration-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&tmp).unwrap();

    let base = [
        ("XDG_STATE_HOME", tmp.to_str().unwrap()),
        ("LECOO_DEVICE", "/dev/null"),
    ];
    let short_device = [
        ("XDG_STATE_HOME", tmp.to_str().unwrap()),
        ("LECOO_DEVICE", "/nonexistent/lecoo-test"),
    ];

    // A kernel ENOTTY must reach stderr unchanged.
    let (ok, _, stderr) = spawn(&base, &["info", "--json"]);
    assert!(!ok, "info must fail on /dev/null");
    assert!(stderr.contains("os error 25"), "stderr: {stderr}");

    // Truthful capability report.
    let (ok, stdout, _) = spawn(&base, &["capabilities", "--json"]);
    assert!(ok, "capabilities must succeed");
    assert!(stdout.contains("\"custom_rgb\":\"unverified\""));
    assert!(stdout.contains("\"side_control\":\"unsupported\""));
    assert!(!stdout.contains("fixed_side_palette"));

    // Semantics that shipped without effect stay rejected before any
    // device access happens.
    for command in ["side", "side-next", "raw", "reset"] {
        let (ok, _, stderr) = spawn(&short_device, &[command]);
        assert!(
            !ok && stderr.contains("unknown command") && !stderr.contains("device_open"),
            "{command}: {stderr}"
        );
    }

    let _ = fs::remove_dir_all(&tmp);
}
