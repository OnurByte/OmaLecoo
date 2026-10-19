//! Persisted state: the saved manual lighting settings and the automation
//! rules. Both live under `$XDG_STATE_HOME/lecoo-gk301` and are written
//! atomically.

use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::protocol::{BRIGHTNESS_MAX, EFFECT_NAMES, PANEL_EFFECTS, SELECT_MAX, SPEED_MAX};

/// The saved manual lighting settings — the user's restore point.
#[derive(Clone)]
pub struct State {
    /// Index of the main-panel effect.
    pub effect: usize,
    /// Panel brightness, 0–13.
    pub brightness: u8,
    /// Effect speed, 0–2.
    pub speed: u8,
    /// Gaming M1/M2 selection, 0–2.
    pub select: u8,
}

impl Default for State {
    fn default() -> Self {
        // Matches the Windows app defaults: brightness 5, speed 2, first option.
        Self {
            effect: 0,
            brightness: 5,
            speed: 2,
            select: 0,
        }
    }
}

impl State {
    /// The state directory shared by the controller, the service and the panel.
    pub fn dir() -> PathBuf {
        if let Ok(dir) = env::var("XDG_STATE_HOME") {
            if !dir.is_empty() {
                return PathBuf::from(dir).join("lecoo-gk301");
            }
        }
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".local/state/lecoo-gk301")
    }

    /// Path of the saved manual settings.
    pub fn path() -> PathBuf {
        Self::dir().join("state.json")
    }

    /// Path of a sibling file in the state directory.
    pub fn sibling(name: &str) -> PathBuf {
        Self::dir().join(name)
    }

    /// Loads the saved settings, falling back to the defaults.
    pub fn load() -> Self {
        let text = match fs::read_to_string(Self::path()) {
            Ok(t) => t,
            Err(_) => return Self::default(),
        };
        let mut st = Self::default();
        if let Some(v) = find_u64(&text, "effect") {
            st.effect = (v as usize) % PANEL_EFFECTS;
        }
        if let Some(v) = find_u64(&text, "brightness") {
            st.brightness = bounded_value(v, BRIGHTNESS_MAX);
        }
        if let Some(v) = find_u64(&text, "speed") {
            st.speed = bounded_value(v, SPEED_MAX);
        }
        if let Some(v) = find_u64(&text, "select") {
            st.select = bounded_value(v, SELECT_MAX);
        }
        st
    }

    /// Saves the settings as the manual restore point.
    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
            .map_err(|e| format!("device command sent but state could not be saved: {}", e))
    }

    /// Writes the settings atomically to an explicit path.
    pub fn save_to(&self, path: &std::path::Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, format!("{}\n", self.json()))?;
        fs::rename(&tmp, path)
    }

    /// The recovered name of the selected effect.
    pub fn name(&self) -> &'static str {
        EFFECT_NAMES[self.effect]
    }

    /// The JSON state document.
    pub fn json(&self) -> String {
        format!(
            "{{\"effect\":{},\"name\":\"{}\",\"brightness\":{},\"speed\":{},\"select\":{},\"panel_count\":{},\"panel_brightness_max\":{},\"speed_max\":{},\"select_max\":{}}}",
            self.effect,
            self.name(),
            self.brightness,
            self.speed,
            self.select,
            PANEL_EFFECTS,
            BRIGHTNESS_MAX,
            SPEED_MAX,
            SELECT_MAX
        )
    }

    /// Rejects values outside the recovered hardware ranges.
    pub fn validate(&self) -> Result<(), String> {
        if self.effect >= PANEL_EFFECTS {
            return Err(format!("panel effect must be in 0..{}", PANEL_EFFECTS - 1));
        }
        if self.brightness > BRIGHTNESS_MAX {
            return Err(format!("panel brightness must be in 0..{}", BRIGHTNESS_MAX));
        }
        if self.speed > SPEED_MAX {
            return Err(format!("speed must be in 0..{}", SPEED_MAX));
        }
        if self.select > SELECT_MAX {
            return Err(format!("selection must be in 0..{}", SELECT_MAX));
        }
        Ok(())
    }
}

/// The automation rules owned by the Signal Engine.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    /// Whether the service may act on desktop events.
    pub automation_enabled: bool,
    /// Whether one restore is sent after the keyboard reconnects.
    pub restore_on_reconnect: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            automation_enabled: false,
            restore_on_reconnect: false,
        }
    }
}

impl Rules {
    /// Path of the rules document.
    pub fn path() -> PathBuf {
        State::sibling("rules.json")
    }

    /// Loads the rules; malformed documents fall back to all-disabled.
    pub fn load() -> Self {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    /// Parses the rules document, rejecting anything unexpected.
    pub fn parse(text: &str) -> Option<Self> {
        let body = text.trim().strip_prefix('{')?.strip_suffix('}')?.trim();
        if body.is_empty() {
            return Some(Self::default());
        }
        let mut rules = Self::default();
        let mut seen_automation = false;
        let mut seen_restore = false;
        for field in body.split(',') {
            let (key, value) = field.split_once(':')?;
            let key = key.trim().strip_prefix('"')?.strip_suffix('"')?;
            let enabled = match value.trim() {
                "true" => true,
                "false" => false,
                _ => return None,
            };
            match key {
                "automation_enabled" if !seen_automation => {
                    seen_automation = true;
                    rules.automation_enabled = enabled;
                }
                "restore_on_reconnect" if !seen_restore => {
                    seen_restore = true;
                    rules.restore_on_reconnect = enabled;
                }
                _ => return None,
            }
        }
        Some(rules)
    }

    /// The JSON rules document.
    pub fn json(self) -> String {
        format!(
            "{{\"automation_enabled\":{},\"restore_on_reconnect\":{}}}",
            self.automation_enabled, self.restore_on_reconnect
        )
    }

    /// Writes the rules atomically.
    pub fn save(self) -> io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, format!("{}\n", self.json()))?;
        fs::rename(tmp, path)
    }
}

fn bounded_value(value: u64, max: u8) -> u8 {
    value.min(max as u64) as u8
}

fn find_u64(text: &str, key: &str) -> Option<u64> {
    let pat = format!("\"{}\"", key);
    let start = text.find(&pat)? + pat.len();
    let rest = &text[start..];
    let colon = rest.find(':')?;
    let rest = rest[colon + 1..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}
