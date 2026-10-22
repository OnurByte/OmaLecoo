//! The GK301 function keys and the Hyprland fragment that programs them.
//!
//! The original Windows tool let the user assign actions to the keyboard's
//! function and media keys through its `config.xml`. On Linux those keys are
//! standard HID consumer codes and the compositor owns them, so the
//! equivalent is a Hyprland binding. The controller keeps the key map and
//! the user's commands, and writes a small Lua fragment that the user
//! includes from their own Hyprland config.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::state::State;

/// Name of the key document inside the state directory.
pub const KEY_FILE: &str = "keys.json";
/// Name of the generated Hyprland fragment inside the state directory.
pub const FRAGMENT_FILE: &str = "hypr-binds.lua";

/// The GK301's special keys, as the original `config.xml` listed them.
///
/// The name and the Windows code come from the original tool. The Linux key
/// is the HID consumer code Hyprland sees, and the default command is the
/// matching Omarchy action.
pub const DEFAULT_KEYS: [(&str, &str, &str, &str); 13] = [
    (
        "Volume-",
        "174",
        "XF86AudioLowerVolume",
        "omarchy-audio-output-volume lower",
    ),
    (
        "Volume+",
        "175",
        "XF86AudioRaiseVolume",
        "omarchy-audio-output-volume raise",
    ),
    (
        "Mute",
        "173",
        "XF86AudioMute",
        "omarchy-audio-output-volume mute-toggle",
    ),
    (
        "Prev Track",
        "177",
        "XF86AudioPrev",
        "omarchy-shell media previous",
    ),
    (
        "Play/Pause",
        "179",
        "XF86AudioPlay",
        "omarchy-shell media playPause",
    ),
    (
        "Next Track",
        "176",
        "XF86AudioNext",
        "omarchy-shell media next",
    ),
    ("Email", "10002", "XF86Mail", ""),
    ("Calculator", "10000", "XF86Calculator", "omacalc"),
    ("www back", "166", "XF86BrowserBack", ""),
    ("www forward", "167", "XF86BrowserForward", ""),
    ("www stop", "169", "XF86BrowserStop", ""),
    ("www refresh", "168", "XF86BrowserRefresh", ""),
    ("My PC", "10001", "XF86Explorer", "omarchy-launch-nautilus"),
];

/// One programmable key.
#[derive(Clone, Debug)]
pub struct KeyEntry {
    /// The name in the original tool.
    pub name: String,
    /// The virtual key code in the original tool.
    pub code: String,
    /// The key Hyprland sees.
    pub linux_key: String,
    /// The command this key runs. Empty means the key is not programmed.
    pub command: String,
}

impl KeyEntry {
    /// The JSON form of an entry.
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "code": self.code,
            "linux_key": self.linux_key,
            "command": self.command,
        })
    }
}

/// Path of the key document.
pub fn path() -> PathBuf {
    State::sibling(KEY_FILE)
}

/// Path of the generated Hyprland fragment.
pub fn fragment_path() -> PathBuf {
    State::sibling(FRAGMENT_FILE)
}

/// The default key map.
pub fn defaults() -> Vec<KeyEntry> {
    DEFAULT_KEYS
        .iter()
        .map(|(name, code, linux_key, command)| KeyEntry {
            name: (*name).to_string(),
            code: (*code).to_string(),
            linux_key: (*linux_key).to_string(),
            command: (*command).to_string(),
        })
        .collect()
}

/// Loads the key map, applying the user's saved commands over the defaults.
pub fn load() -> Vec<KeyEntry> {
    let mut entries = defaults();
    let text = match fs::read_to_string(path()) {
        Ok(text) => text,
        Err(_) => return entries,
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => return entries,
    };
    let saved = match value.get("keys").and_then(Value::as_array) {
        Some(items) => items,
        None => return entries,
    };
    for item in saved {
        let (Some(name), Some(command)) = (
            item.get("name").and_then(Value::as_str),
            item.get("command").and_then(Value::as_str),
        ) else {
            continue;
        };
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
        {
            entry.command = command.to_string();
        }
    }
    entries
}

/// Stores the key map.
pub fn save(entries: &[KeyEntry]) -> io::Result<()> {
    let path = path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let document = json!({ "keys": entries.iter().map(KeyEntry::to_json).collect::<Vec<_>>() });
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&tmp, format!("{}\n", document))?;
    fs::rename(tmp, path)
}

/// Sets the command of one key, reporting whether the key exists.
pub fn set_command(entries: &mut [KeyEntry], name: &str, command: &str) -> Result<(), String> {
    let entry = entries
        .iter_mut()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("unknown key: {}", name))?;
    entry.command = command.trim().to_string();
    Ok(())
}

/// Quotes a Lua short string, including control bytes and Windows paths.
fn lua_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii_control() => out.push_str(&format!("\\{:03}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Builds the Hyprland fragment for the current key map.
pub fn fragment(entries: &[KeyEntry]) -> String {
    let mut out = String::new();
    out.push_str("-- OmaLecoo — Lecoo GK301 (SEMICO 1a2c:9df4) function keys.\n");
    out.push_str("-- Generated by `lecoo-gk301 keys export`.\n");
    out.push_str("--\n");
    out.push_str("-- Include it from your Hyprland config (~/.config/hypr/hyprland.lua):\n");
    out.push_str(&format!("--   dofile(\"{}\")\n", fragment_path().display()));
    out.push_str("--\n");
    out.push_str("-- The key names and codes come from the original Windows tool's config.xml.\n");
    out.push_str("-- Omarchy already binds the volume and media keys; including this file\n");
    out.push_str("-- takes them over. Remove a line to keep Omarchy's own binding.\n\n");
    for entry in entries {
        if entry.command.is_empty() {
            out.push_str(&format!(
                "-- {} ({}) is not programmed. Set one with: lecoo-gk301 keys set \"{}\" --command \"<command>\"\n",
                entry.name, entry.linux_key, entry.name
            ));
            continue;
        }
        out.push_str(&format!(
            "o.bind({}, {}, {})\n",
            lua_string(&entry.linux_key),
            lua_string(&format!("{} ({})", entry.name, entry.code)),
            lua_string(&entry.command)
        ));
    }
    out
}

/// Writes the Hyprland fragment and returns its path.
pub fn export(entries: &[KeyEntry], out: Option<&str>) -> Result<PathBuf, String> {
    let target = match out {
        Some(path) => PathBuf::from(path),
        None => fragment_path(),
    };
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {}", parent.display(), e))?;
    }
    fs::write(&target, fragment(entries))
        .map_err(|e| format!("could not write {}: {}", target.display(), e))?;
    Ok(target)
}
