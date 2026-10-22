use std::{fs, process::Command};

#[test]
fn key_commands_are_escaped_for_lua() {
    let mut entries = omalecoo::keys::defaults();
    omalecoo::keys::set_command(
        &mut entries,
        "Calculator",
        "notify-send \"hello\"\\path\nnext\r\tz",
    )
    .unwrap();
    let fragment = omalecoo::keys::fragment(&entries);
    assert!(fragment.contains(r#"o.bind("XF86Calculator", "Calculator (10000)", "notify-send \"hello\"\\path\nnext\r\tz")"#), "{fragment}");
}

#[test]
fn clear_state_removes_all_owned_settings_and_preserves_other_files() {
    let dir = std::env::temp_dir().join(format!("omalecoo-cleanup-{}", std::process::id()));
    let state = dir.join("lecoo-gk301");
    fs::create_dir_all(&state).unwrap();
    let owned = [
        "state.json",
        "rules.json",
        "macros.json",
        "keys.json",
        "hypr-binds.lua",
        "blackbox.jsonl",
        "blackbox.jsonl.1",
        "debug.log",
        "debug.log.1",
    ];
    for name in owned {
        fs::write(state.join(name), "test").unwrap();
    }
    fs::write(state.join("unrelated"), "keep").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_lecoo-gk301"))
        .env("XDG_STATE_HOME", &dir)
        .args(["state", "clear"])
        .output()
        .unwrap();
    assert!(out.status.success());
    for name in owned {
        assert!(!state.join(name).exists(), "left {name} behind");
    }
    assert_eq!(fs::read_to_string(state.join("unrelated")).unwrap(), "keep");
    assert!(state.join("command.lock").exists());
    fs::remove_dir_all(dir).unwrap();
}
