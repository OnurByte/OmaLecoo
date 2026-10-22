//! Macros: named sequences of lighting states that the controller plays on
//! the keyboard.
//!
//! The original Windows tool recorded key sequences and replayed them on the
//! Windows host with `keybd_event`. On Linux the compositor owns keyboard
//! input, so the device-side equivalent is what the hardware can actually
//! do: a timed sequence of lighting commands. A macro is stored in
//! `macros.json` next to the other OmaLecoo state and is played under the
//! same process lock as every other writer.

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use crate::device::{send_panel, Device};
use crate::protocol::{BRIGHTNESS_MAX, PANEL_EFFECTS, SELECT_MAX, SPEED_MAX};
use crate::state::State;

/// Name of the macro document inside the state directory.
pub const MACRO_FILE: &str = "macros.json";

/// Longest macro name or group the panel's listing rows are sized for.
const TEXT_LIMIT: usize = 80;

/// Resource budgets shared by import, persistence and the panel listing.
pub const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
pub const MAX_MACROS: usize = 64;
pub const MAX_STEPS_PER_MACRO: usize = 256;
pub const MAX_TOTAL_STEPS: usize = 4096;

/// Reads at most one byte beyond the budget, including for growing files.
pub fn read_document(path: &std::path::Path) -> Result<Vec<Macro>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take((MAX_DOCUMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(format!(
            "macro document exceeds {} bytes",
            MAX_DOCUMENT_BYTES
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let items = match value.get("macros") {
        Some(items) => items
            .as_array()
            .ok_or("macros must be an array")?
            .as_slice(),
        None => std::slice::from_ref(&value),
    };
    if items.len() > MAX_MACROS {
        return Err(format!("collection exceeds {} macros", MAX_MACROS));
    }
    // Count raw steps before filtering invalid entries; invalid input cannot
    // hide a resource-budget violation.
    let mut total = 0;
    for item in items {
        if let Some(steps) = item.get("steps").and_then(Value::as_array) {
            if steps.len() > MAX_STEPS_PER_MACRO {
                return Err(format!("macro exceeds {} steps", MAX_STEPS_PER_MACRO));
            }
            total += steps.len();
            if total > MAX_TOTAL_STEPS {
                return Err(format!(
                    "collection exceeds {} total steps",
                    MAX_TOTAL_STEPS
                ));
            }
        }
    }
    Ok(items.iter().filter_map(Macro::from_json).collect())
}

fn validate_collection(macros: &[Macro]) -> io::Result<()> {
    if macros.len() > MAX_MACROS
        || macros.iter().any(|m| m.steps.len() > MAX_STEPS_PER_MACRO)
        || macros.iter().map(|m| m.steps.len()).sum::<usize>() > MAX_TOTAL_STEPS
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "macro collection budget exceeded (64 macros, 256 steps per macro, 4096 total steps)",
        ));
    }
    Ok(())
}

/// Normalizes a name or group coming from a macro document: control
/// characters are removed and the text is capped before it is stored. The
/// panel renders these strings as plain text, and every other consumer of
/// the stored document inherits the same bounded form.
pub fn clean_text(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(TEXT_LIMIT)
        .collect::<String>()
        .trim()
        .to_string()
}

/// One lighting step of a macro.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// Delay before this step is applied, in milliseconds.
    pub delay_ms: u64,
    /// Panel effect index.
    pub effect: usize,
    /// Panel brightness.
    pub brightness: u8,
    /// Effect speed.
    pub speed: u8,
    /// Gaming key selection.
    pub select: u8,
}

impl Step {
    /// The JSON form of a step.
    pub fn to_json(&self) -> Value {
        json!({
            "delay_ms": self.delay_ms,
            "effect": self.effect,
            "brightness": self.brightness,
            "speed": self.speed,
            "select": self.select,
        })
    }

    /// Reads a step, rejecting values outside the recovered hardware ranges.
    pub fn from_json(value: &Value) -> Option<Step> {
        let delay_ms = value.get("delay_ms").and_then(Value::as_u64).unwrap_or(0);
        let effect = value.get("effect").and_then(Value::as_u64)? as usize;
        let brightness = value.get("brightness").and_then(Value::as_u64)? as u8;
        let speed = value.get("speed").and_then(Value::as_u64)? as u8;
        let select = value.get("select").and_then(Value::as_u64)? as u8;
        if effect >= PANEL_EFFECTS
            || brightness > BRIGHTNESS_MAX
            || speed > SPEED_MAX
            || select > SELECT_MAX
        {
            return None;
        }
        Some(Step {
            delay_ms,
            effect,
            brightness,
            speed,
            select,
        })
    }

    /// The lighting state this step applies.
    pub fn state(&self) -> State {
        State {
            effect: self.effect,
            brightness: self.brightness,
            speed: self.speed,
            select: self.select,
        }
    }
}

/// A named macro.
#[derive(Clone, Debug)]
pub struct Macro {
    /// The name shown in the panel and used on the command line.
    pub name: String,
    /// The group this macro belongs to, like the original tool's macro groups.
    pub group: String,
    /// How many times the sequence is repeated.
    pub repeat: u32,
    /// The steps of the sequence.
    pub steps: Vec<Step>,
}

impl Macro {
    /// The JSON form of a macro.
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "group": self.group,
            "repeat": self.repeat,
            "steps": self.steps.iter().map(Step::to_json).collect::<Vec<_>>(),
        })
    }

    /// Reads a macro, rejecting malformed documents. The name and group pass
    /// through `clean_text`, so a third-party document cannot carry control
    /// characters or unbounded strings into the panel and the shell process.
    pub fn from_json(value: &Value) -> Option<Macro> {
        let name = clean_text(value.get("name")?.as_str()?);
        if name.is_empty() {
            return None;
        }
        let group = clean_text(
            value
                .get("group")
                .and_then(Value::as_str)
                .unwrap_or("Group 0"),
        );
        let group = if group.is_empty() {
            "Group 0".to_string()
        } else {
            group
        };
        let repeat = value
            .get("repeat")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .clamp(1, 64) as u32;
        let items = value.get("steps")?.as_array()?;
        if items.len() > MAX_STEPS_PER_MACRO {
            return None;
        }
        let steps = items
            .iter()
            .map(Step::from_json)
            .collect::<Option<Vec<_>>>()?;
        Some(Macro {
            name,
            group,
            repeat,
            steps,
        })
    }
}

/// Path of the macro document.
pub fn path() -> PathBuf {
    State::sibling(MACRO_FILE)
}

/// Loads every stored macro. A malformed document yields an empty list.
pub fn load() -> Vec<Macro> {
    read_document(&path()).unwrap_or_default()
}

/// Stores the macro list.
pub fn save(macros: &[Macro]) -> io::Result<()> {
    validate_collection(macros)?;
    let document = json!({ "macros": macros.iter().map(Macro::to_json).collect::<Vec<_>>() });
    let text = format!("{}\n", document);
    if text.len() > MAX_DOCUMENT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "macro document exceeds byte budget",
        ));
    }
    let path = path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&tmp, text)?;
    fs::rename(tmp, path)
}

/// Finds a macro by name.
pub fn find<'a>(macros: &'a [Macro], name: &str) -> Option<&'a Macro> {
    macros
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case(name))
}

/// Replaces the macro with the same name, or appends it.
pub fn upsert(macros: &mut Vec<Macro>, macro_def: Macro) {
    if let Some(existing) = macros
        .iter_mut()
        .find(|item| item.name.eq_ignore_ascii_case(&macro_def.name))
    {
        *existing = macro_def;
        return;
    }
    macros.push(macro_def);
}

/// Removes a macro by name, reporting whether it existed.
pub fn remove(macros: &mut Vec<Macro>, name: &str) -> bool {
    let before = macros.len();
    macros.retain(|item| !item.name.eq_ignore_ascii_case(name));
    macros.len() != before
}

/// Plays a macro on the connected keyboard.
pub fn play(macro_def: &Macro, device_arg: Option<&str>, origin: &str) -> Result<(), String> {
    if macro_def.steps.is_empty() {
        return Err(format!("macro '{}' has no steps", macro_def.name));
    }
    let mut device: Option<Device> = None;
    for _ in 0..macro_def.repeat.max(1) {
        for step in &macro_def.steps {
            if step.delay_ms > 0 {
                thread::sleep(Duration::from_millis(step.delay_ms));
            }
            let dev = match device.as_mut() {
                Some(dev) => dev,
                None => {
                    device = Some(Device::open_detected(device_arg)?);
                    device.as_mut().expect("device was just opened")
                }
            };
            send_panel(dev, &step.state(), origin)?;
            // The same pacing every other writer keeps, so a macro cannot
            // flood the keyboard with back-to-back reports.
            thread::sleep(Duration::from_millis(crate::commands::PROTOCOL_PACING_MS));
        }
    }
    Ok(())
}

/// Parses a JSON array of steps, as the command line and the panel pass it.
pub fn parse_steps(text: &str) -> Result<Vec<Step>, String> {
    if text.len() > MAX_DOCUMENT_BYTES {
        return Err("--steps exceeds byte budget".into());
    }
    let value: Value = serde_json::from_str(text.trim())
        .map_err(|e| format!("--steps must be a JSON array: {}", e))?;
    let items = value
        .as_array()
        .ok_or("--steps must be a JSON array of step objects")?;
    if items.len() > MAX_STEPS_PER_MACRO {
        return Err(format!("--steps exceeds {} steps", MAX_STEPS_PER_MACRO));
    }
    let mut steps = Vec::new();
    for item in items {
        steps.push(Step::from_json(item).ok_or("--steps contains an invalid step value")?);
    }
    if steps.is_empty() {
        return Err("--steps needs at least one step".into());
    }
    Ok(steps)
}
