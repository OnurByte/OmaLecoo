//! White-box transport checks through the C interception shim
//! (`tests/ioctl_capture.c`): one main-panel packet per operation, the
//! validated firmware response and its failure matrix. `/dev/null` only;
//! no USB hardware.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn transport_shim_checks() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let tmp = std::env::temp_dir().join(format!(
        "omalecoo-transport-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&tmp).unwrap();

    let so = tmp.join("capture.so");
    let source = Path::new(manifest).join("tests/ioctl_capture.c");
    let compile = Command::new("cc")
        .args(["-shared", "-fPIC"])
        .arg(&source)
        .arg("-o")
        .arg(&so)
        .status()
        .expect("cc must be available to build the interception shim");
    assert!(compile.success(), "shim compile failed");

    let trace_path = tmp.join("trace");
    let env = [
        ("LD_PRELOAD", so.to_str().unwrap()),
        ("LECOO_TEST_TRACE", trace_path.to_str().unwrap()),
        ("LECOO_DEVICE", "/dev/null"),
        ("XDG_STATE_HOME", tmp.to_str().unwrap()),
    ];

    // One packet line per traced SET_FEATURE, coming from the shim's
    // trace file — not from process output.
    let packets = |args: &[&str]| -> Vec<String> {
        fs::write(&trace_path, b"").unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_lecoo-gk301"))
            .args(args)
            .envs(env.iter().copied())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("spawn the CLI");
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&fs::read(&trace_path).expect("trace file"))
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_owned)
            .collect()
    };

    // Single main-panel packet per operation, panel path only.
    for args in [vec!["preset", "night"], vec!["restore"]] {
        for flags in [Vec::new(), vec!["--panel-only"]] {
            let mut full = args.clone();
            full.extend_from_slice(&flags);
            let trace = packets(&full);
            assert_eq!(trace.len(), 1, "{full:?}: {trace:?}");
            assert!(trace[0].starts_with("07 FF FF "), "{trace:?}");
            assert_ne!(trace[0].split_whitespace().nth(3), Some("20"));
        }
    }

    let output = Command::new(env!("CARGO_BIN_EXE_lecoo-gk301"))
        .args(["response", "--json"])
        .envs(env.iter().copied())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn the CLI");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    for expected in [
        "\"device\":\"/dev/null\"",
        "\"request\":\"07 F1 00 00 00 00 00 00\"",
        "\"raw\":\"07 01 16 00 00 00 00 00\"",
        "\"firmware\":\"1.16\"",
        "\"kind\":\"firmware\"",
        "\"lighting_readback\":false",
        "\"roundtrip_ms\":",
    ] {
        assert!(stdout.contains(expected), "missing {expected}: {stdout}");
    }

    // Validated response path: every tampered fixture must fail.
    for fixture in ["short", "bad-id", "empty", "bad-version"] {
        let out = Command::new(env!("CARGO_BIN_EXE_lecoo-gk301"))
            .args(["response", "--json"])
            .envs(env.iter().copied())
            .env("LECOO_TEST_RESPONSE", fixture)
            .output()
            .expect("spawn the CLI");
        assert!(!out.status.success(), "fixture {fixture} must fail");
    }
    let out = Command::new(env!("CARGO_BIN_EXE_lecoo-gk301"))
        .args(["response", "--json"])
        .envs(env.iter().copied())
        .env("LECOO_TEST_SHORT_SET", "1")
        .output()
        .expect("spawn the CLI");
    assert!(!out.status.success(), "short set must fail");

    let _ = fs::remove_dir_all(&tmp);
}
