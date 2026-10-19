//! The command implementations behind one dispatch function.
//!
//! Every command runs under the shared process lock. Commands that write to
//! the keyboard respect the recovered 110 ms protocol pacing; local metadata
//! commands do not.

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::blackbox::blackbox_json;
use crate::cli::{device_arg, has_flag, opt_switch, opt_u8, usage};
use crate::device::{
    detect_device, device_presence_json, path_is_writable, query_firmware, send_panel, Device,
};
use crate::keys;
use crate::logging::debug_log;
use crate::macros;
use crate::probe::{probe_brightness, probe_startup};
use crate::protocol::{capabilities_json, catalog_json, hex, EFFECT_NAMES, PANEL_EFFECTS};
use crate::state::{Rules, State};
use crate::udev::udev_command;

/// Commands that never write to the keyboard, so they skip the protocol
/// pacing delay.
const NO_PACING_COMMANDS: [&str; 14] = [
    "status",
    "catalog",
    "list",
    "help",
    "-h",
    "--help",
    "dev",
    "capabilities",
    "probe-brightness",
    "probe-startup",
    "rules",
    "blackbox",
    "state",
    "udev",
];

/// The minimum delay between two HID commands, as recovered from the
/// Windows sender's shared gate.
pub const PROTOCOL_PACING_MS: u64 = 110;

/// Runs one command. `args` excludes the program name.
pub fn dispatch(args: &[String]) -> Result<(), String> {
    let Some(cmd) = args.first().map(|s| s.as_str()) else {
        usage();
        return Ok(());
    };
    let origin = crate::cli::command_origin(args)?;
    let device = device_arg(args);

    debug_log(&format!("command={:?}", args));
    // Windows sender enforces a shared 100 ms gate (0x41feb0/0x41ffd0).
    // Delay under the process lock so separate invocations obey it too.
    if !NO_PACING_COMMANDS.contains(&cmd) {
        thread::sleep(Duration::from_millis(PROTOCOL_PACING_MS));
    }
    match cmd {
        "capabilities" => println!("{}", capabilities_json()),
        "doctor" => {
            let dev = Device::open_detected(device)?;
            println!("device: {}", dev.path().display());
            println!("state: {}", State::path().display());
            println!("debug: {}", State::sibling("debug.log").display());
            let resp = query_firmware(&dev, origin)?;
            println!("request: {}", hex(&crate::protocol::FIRMWARE_REQUEST));
            println!(
                "firmware report: {} (firmware {})",
                hex(&resp.raw),
                resp.firmware
            );
            println!("Firmware response received. This query does not confirm that lighting settings were applied; the readback does not carry lighting state.");
        }
        "restore" => {
            // `--factory` applies the same default record the original Windows
            // tool starts with: Steady, brightness 5, speed 2, first option.
            let factory = has_flag(args, "--factory");
            let st = if factory {
                State::default()
            } else {
                State::load()
            };
            let dev = Device::open_detected(device)?;
            send_panel(&dev, &st, origin)?;
            if factory {
                // A manual factory reset becomes the new restore point; an
                // automatic write must not replace the user's manual state.
                if origin != "signal" {
                    st.save()?;
                }
                println!(
                    "Factory defaults applied: {} brightness {}/13 speed {}/2",
                    st.name(),
                    st.brightness,
                    st.speed
                );
            } else {
                println!("Last saved lighting settings applied");
            }
        }
        "response" => {
            let dev = Device::open_detected(device)?;
            let resp = query_firmware(&dev, origin)?;
            print_response(&resp, has_flag(args, "--json") || has_flag(args, "-j"));
        }
        "help" | "-h" | "--help" => {
            usage();
        }
        "list" => {
            for (i, name) in EFFECT_NAMES.iter().enumerate() {
                println!("{:2}  {}", i, name);
            }
        }
        "catalog" => {
            if has_flag(args, "--json") || has_flag(args, "-j") {
                println!("{}", catalog_json());
            } else {
                for (i, name) in EFFECT_NAMES.iter().enumerate() {
                    println!("panel {:2}  {}", i, name);
                }
            }
        }
        "dev" => {
            let path = detect_device();
            if has_flag(args, "--json") {
                println!("{}", device_presence_json());
                return Ok(());
            }
            match path {
                Some(p) => {
                    println!("{}", p.display());
                    println!("writable: {}", path_is_writable(&p));
                }
                None => {
                    println!("not found (1a2c:9df4 interface 1)");
                }
            }
        }
        "rules" => {
            if args.get(1).map(String::as_str) == Some("set") {
                let automation = opt_switch(args, "--automation")?;
                let restore = opt_switch(args, "--restore-on-reconnect")?;
                let rules = Rules {
                    automation_enabled: automation.ok_or("missing --automation <on|off>")?,
                    restore_on_reconnect: restore
                        .ok_or("missing --restore-on-reconnect <on|off>")?,
                };
                rules
                    .save()
                    .map_err(|e| format!("could not save rules: {}", e))?;
                println!("{}", rules.json());
            } else {
                println!("{}", Rules::load().json());
            }
        }
        "blackbox" => {
            if !has_flag(args, "--json") {
                return Err("usage: lecoo-gk301 blackbox --json".into());
            }
            println!("{}", blackbox_json());
        }
        "state" => {
            if args.get(1).map(String::as_str) != Some("clear") {
                return Err("usage: lecoo-gk301 state clear".into());
            }
            clear_state()?;
        }
        "udev" => {
            let action = args
                .get(1)
                .map(String::as_str)
                .ok_or("usage: lecoo-gk301 udev install|remove")?;
            udev_command(action)?;
        }
        "status" => {
            let st = State::load();
            if has_flag(args, "--json") {
                println!("{}", st.json());
            } else {
                println!("effect    : {} ({})", st.effect, st.name());
                println!("brightness: {}/13", st.brightness);
                println!("speed     : {}/2", st.speed);
                println!("selection : {}/2", st.select);
                println!("(settings are the last values sent from this computer)");
            }
        }
        "next" | "prev" => {
            let mut st = State::load();
            st.effect = if cmd == "next" {
                (st.effect + 1) % PANEL_EFFECTS
            } else {
                (st.effect + PANEL_EFFECTS - 1) % PANEL_EFFECTS
            };
            let dev = Device::open_detected(device)?;
            send_panel(&dev, &st, origin)?;
            // Automatic rules must preserve the user's manual restore point.
            if origin != "signal" {
                st.save()?;
            }
            println!(
                "panel: {} ({}) brightness {}/13 speed {}/2",
                st.effect,
                st.name(),
                st.brightness,
                st.speed
            );
        }
        "panel" => {
            let idx = args
                .get(1)
                .ok_or("usage: lecoo-gk301 panel <0-19>")?
                .parse::<usize>()
                .map_err(|_| "effect index must be a number".to_string())?;
            let mut st = State::load();
            st.effect = idx;
            if let Some(v) = opt_u8(args, "--brightness")? {
                st.brightness = v;
            }
            if let Some(v) = opt_u8(args, "--speed")? {
                st.speed = v;
            }
            if let Some(v) = opt_u8(args, "--select")? {
                st.select = v;
            }
            st.validate()?;
            let dev = Device::open_detected(device)?;
            send_panel(&dev, &st, origin)?;
            if origin != "signal" {
                st.save()?;
            }
            println!(
                "panel: {} ({}) brightness {}/13 speed {}/2 selection {}",
                st.effect,
                st.name(),
                st.brightness,
                st.speed,
                st.select
            );
        }
        "brightness" => {
            let v = args
                .get(1)
                .ok_or("usage: lecoo-gk301 brightness <0-13>")?
                .parse::<u8>()
                .map_err(|_| "brightness must be a number".to_string())?;
            let mut st = State::load();
            st.brightness = v;
            st.validate()?;
            let dev = Device::open_detected(device)?;
            send_panel(&dev, &st, origin)?;
            if origin != "signal" {
                st.save()?;
            }
            println!("brightness: {}/13", st.brightness);
        }
        "preset" => {
            let name = args
                .get(1)
                .ok_or("usage: lecoo-gk301 preset <silent|game|night|party|off|random>")?;
            let st = preset_state(name)?;
            let dev = Device::open_detected(device)?;
            send_panel(&dev, &st, origin)?;
            if origin != "signal" {
                st.save()?;
            }
            println!(
                "preset: {} · panel {} ({}) {}/13 speed {}/2",
                name,
                st.effect,
                st.name(),
                st.brightness,
                st.speed
            );
        }
        "info" => {
            let dev = Device::open_detected(device)?;
            dev.set_feature([0xF1, 0, 0, 0, 0, 0, 0], origin)
                .map_err(|e| {
                    format!(
                        "could not send query: {} ({})",
                        e,
                        crate::device::permission_hint(&e)
                    )
                })?;
            thread::sleep(Duration::from_millis(50));
            let resp = dev.get_feature(origin).map_err(|e| {
                format!(
                    "could not read response: {} ({})",
                    e,
                    crate::device::permission_hint(&e)
                )
            })?;
            let major = resp[1];
            let minor = resp[2];
            let firmware = if major <= 9 {
                format!("{}.{:X}{:X}", major, (minor >> 4) & 0x0F, minor & 0x0F)
            } else {
                String::new()
            };
            if has_flag(args, "--json") || has_flag(args, "-j") {
                println!(
                    "{{\"raw\":\"{}\",\"firmware\":\"{}\"}}",
                    hex(&resp),
                    firmware
                );
            } else {
                println!("raw response: {}", hex(&resp));
                if !firmware.is_empty() {
                    println!("firmware: {}", firmware);
                }
            }
        }
        "get" => {
            let dev = Device::open_detected(device)?;
            let resp = dev.get_feature(origin).map_err(|e| {
                format!(
                    "could not read: {} ({})",
                    e,
                    crate::device::permission_hint(&e)
                )
            })?;
            println!("{}", hex(&resp));
            println!("(this readback carries firmware data, not lighting state)");
        }
        "probe-brightness" => probe_brightness(args, device, origin)?,
        "probe-startup" => probe_startup(args, device, origin)?,
        "macro" => macro_command(args, device, origin)?,
        "keys" => keys_command(args)?,
        "zones" => zones_command(args)?,
        other => {
            return Err(format!("unknown command: {}\n\nlecoo-gk301 --help", other));
        }
    }
    Ok(())
}

/// `macro list|save|remove|play|import|export`
fn macro_command(args: &[String], device: Option<&str>, origin: &str) -> Result<(), String> {
    let action = args
        .get(1)
        .map(String::as_str)
        .ok_or("usage: lecoo-gk301 macro list|save|remove|play|import|export")?;
    let mut macros = macros::load();
    match action {
        "list" => {
            if has_flag(args, "--json") {
                let document = serde_json::json!({
                    "macros": macros.iter().map(macros::Macro::to_json).collect::<Vec<_>>()
                });
                println!("{}", document);
            } else if macros.is_empty() {
                println!("No macros saved");
            } else {
                for item in &macros {
                    println!(
                        "{} · {} · {} step(s) · repeat {}",
                        item.name,
                        item.group,
                        item.steps.len(),
                        item.repeat
                    );
                }
            }
        }
        "save" => {
            let name = match opt_value(args, "--name") {
                Some(name) => name,
                None => match args.get(2).map(String::as_str) {
                    Some(name) if !name.starts_with("--") => name.to_string(),
                    _ => {
                        return Err(
                            "usage: lecoo-gk301 macro save --name <name> --steps <json>".into()
                        )
                    }
                },
            };
            let steps_json = opt_value(args, "--steps")
                .ok_or("macro save needs --steps <json array of steps>")?;
            let steps = macros::parse_steps(&steps_json)?;
            let group = opt_value(args, "--group").unwrap_or_else(|| "Group 0".to_string());
            let repeat = opt_value(args, "--repeat")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(1)
                .clamp(1, 64);
            let macro_def = macros::Macro {
                name: name.trim().to_string(),
                group,
                repeat,
                steps,
            };
            macros::upsert(&mut macros, macro_def);
            macros::save(&macros).map_err(|e| format!("could not save macros: {}", e))?;
            println!("Saved macro: {}", name);
        }
        "remove" => {
            let name = args
                .get(2)
                .map(String::as_str)
                .ok_or("usage: lecoo-gk301 macro remove <name>")?;
            if !macros::remove(&mut macros, name) {
                return Err(format!("no macro named {}", name));
            }
            macros::save(&macros).map_err(|e| format!("could not save macros: {}", e))?;
            println!("Removed macro: {}", name);
        }
        "play" => {
            let name = args
                .get(2)
                .map(String::as_str)
                .ok_or("usage: lecoo-gk301 macro play <name>")?;
            let macro_def = macros::find(&macros, name)
                .ok_or_else(|| format!("no macro named {}", name))?
                .clone();
            macros::play(&macro_def, device, origin)?;
            println!("Played macro: {}", macro_def.name);
        }
        "import" => {
            let file = args
                .get(2)
                .map(String::as_str)
                .ok_or("usage: lecoo-gk301 macro import <file>")?;
            let text = std::fs::read_to_string(file)
                .map_err(|e| format!("could not read {}: {}", file, e))?;
            let value: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| format!("{} is not valid JSON: {}", file, e))?;
            let items = match value.get("macros").and_then(serde_json::Value::as_array) {
                Some(items) => items.clone(),
                None => vec![value.clone()],
            };
            let mut count = 0;
            for item in &items {
                if let Some(macro_def) = macros::Macro::from_json(item) {
                    macros::upsert(&mut macros, macro_def);
                    count += 1;
                }
            }
            if count == 0 {
                return Err(format!("{} contains no usable macro", file));
            }
            macros::save(&macros).map_err(|e| format!("could not save macros: {}", e))?;
            println!("Imported {} macro(s) from {}", count, file);
        }
        "export" => {
            let file = args
                .get(2)
                .map(String::as_str)
                .ok_or("usage: lecoo-gk301 macro export <file>")?;
            let document = serde_json::json!({
                "macros": macros.iter().map(macros::Macro::to_json).collect::<Vec<_>>()
            });
            std::fs::write(file, format!("{}\n", document))
                .map_err(|e| format!("could not write {}: {}", file, e))?;
            println!("Exported {} macro(s) to {}", macros.len(), file);
        }
        other => {
            return Err(format!(
                "unknown macro action: {}\n\nlecoo-gk301 macro list|save|remove|play|import|export",
                other
            ))
        }
    }
    Ok(())
}

/// `keys list|set|export`
fn keys_command(args: &[String]) -> Result<(), String> {
    let action = args
        .get(1)
        .map(String::as_str)
        .ok_or("usage: lecoo-gk301 keys list|set|export")?;
    let mut entries = keys::load();
    match action {
        "list" => {
            if has_flag(args, "--json") {
                let document = serde_json::json!({
                    "keys": entries.iter().map(keys::KeyEntry::to_json).collect::<Vec<_>>()
                });
                println!("{}", document);
            } else {
                for entry in &entries {
                    println!(
                        "{:<14} {:<22} {}",
                        entry.name,
                        entry.linux_key,
                        if entry.command.is_empty() {
                            "(not programmed)"
                        } else {
                            &entry.command
                        }
                    );
                }
            }
        }
        "set" => {
            let name = args
                .get(2)
                .map(String::as_str)
                .ok_or("usage: lecoo-gk301 keys set <name> --command \"<command>\"")?;
            let command = opt_value(args, "--command")
                .ok_or("keys set needs --command \"<command>\" (an empty value clears it)")?;
            keys::set_command(&mut entries, name, &command)?;
            keys::save(&entries).map_err(|e| format!("could not save keys: {}", e))?;
            println!(
                "{} → {}",
                name,
                if command.trim().is_empty() {
                    "(cleared)"
                } else {
                    &command
                }
            );
        }
        "export" => {
            let out = opt_value(args, "--out");
            let target = keys::export(&entries, out.as_deref())?;
            println!("Wrote {}", target.display());
            println!(
                "Include it from your Hyprland config:\n  dofile(\"{}\")",
                target.display()
            );
        }
        other => {
            return Err(format!(
                "unknown keys action: {}\n\nlecoo-gk301 keys list|set|export",
                other
            ))
        }
    }
    Ok(())
}

/// `zones list`
///
/// The keyboard's own lighting zones. They live in the keyboard and the
/// recovered report has no field for them, so this command only lists them.
fn zones_command(args: &[String]) -> Result<(), String> {
    let action = args
        .get(1)
        .map(String::as_str)
        .ok_or("usage: lecoo-gk301 zones list")?;
    match action {
        "list" => {
            if has_flag(args, "--json") {
                println!("{}", crate::zones::list_json());
            } else {
                for zone in crate::zones::list() {
                    println!("{:<6} {:<10} {}", zone.key, zone.name, zone.lights);
                }
                println!("\nThe zone is chosen on the keyboard; the controller cannot set it.");
            }
        }
        other => {
            return Err(format!(
                "unknown zones action: {}\n\nlecoo-gk301 zones list",
                other
            ))
        }
    }
    Ok(())
}

/// Reads the value of an option like `--name value`.
fn opt_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

/// Prints a validated firmware response, as text or as JSON.
pub fn print_response(resp: &crate::device::FirmwareResponse, json: bool) {
    if json {
        println!("{}", crate::device::firmware_report_json(resp));
    } else {
        println!("device: {}", resp.device.display());
        println!("request: {}", hex(&crate::protocol::FIRMWARE_REQUEST));
        println!("response: {}", hex(&resp.raw));
        println!("firmware: {}", resp.firmware);
        println!("roundtrip: {} ms", resp.roundtrip_ms);
        println!("kind: firmware acknowledgment; the readback does not carry lighting state");
    }
}

/// The settings a named preset selects.
pub fn preset_state(name: &str) -> Result<State, String> {
    let mut st = State::load();
    match name {
        "silent" => {
            st.effect = 0;
            st.brightness = 1;
            st.speed = 0;
            st.select = 0;
        }
        "game" => {
            st.effect = 1;
            st.brightness = 5;
            st.speed = 2;
            st.select = 0;
        }
        "night" => {
            st.effect = 2;
            st.brightness = 1;
            st.speed = 0;
            st.select = 0;
        }
        "party" => {
            st.effect = 13;
            st.brightness = 5;
            st.speed = 2;
            st.select = 0;
        }
        "off" => {
            st.brightness = 0;
        }
        "random" => {
            let mut seed = random_seed();
            let mut next = |max: usize| {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((seed >> 33) as usize) % max
            };
            st.effect = next(PANEL_EFFECTS);
            st.brightness = 1 + next(5) as u8;
            st.speed = next(3) as u8;
            st.select = if st.effect == 3 {
                next(3) as u8
            } else {
                next(2) as u8
            };
        }
        other => {
            return Err(format!(
                "unknown preset: {} (silent|game|night|party|off|random)",
                other
            ))
        }
    }
    st.validate()?;
    Ok(st)
}

fn random_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ ((std::process::id() as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15
}

/// Removes the saved OmaLecoo state, leaving the lock and unrelated files.
pub fn clear_state() -> Result<(), String> {
    let state_path = State::path();
    let dir = state_path.parent().unwrap_or_else(|| Path::new("."));
    for name in [
        "state.json",
        "rules.json",
        "blackbox.jsonl",
        "blackbox.jsonl.1",
        "debug.log",
        "debug.log.1",
    ] {
        let path = dir.join(name);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("could not remove {}: {}", path.display(), error)),
        }
    }
    println!("OmaLecoo state cleared");
    Ok(())
}
