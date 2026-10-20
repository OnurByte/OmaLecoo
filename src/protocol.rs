//! Wire-level constants, the recovered effect catalog, and the JSON reports
//! that describe what the keyboard actually supports.

use crate::state::State;

/// Feature report ID used by the GK301 vendor interface.
pub const REPORT_ID: u8 = 0x07;
/// USB vendor ID of the Lecoo GK301.
pub const VID: u32 = 0x1a2c;
/// USB product ID of the Lecoo GK301.
pub const PID: u32 = 0x9df4;
/// Number of recovered main-panel effects.
pub const PANEL_EFFECTS: usize = 20;
/// Highest brightness seen in the recovered saved records. The wire rejects
/// values above `WINDOWS_BRIGHTNESS_MAX` for real lighting — 8, 9 and 13 were
/// each sent and each rendered no light — and the original tool sends 13 only
/// in its start-up record, which behaves as a sync/no-op.
pub const BRIGHTNESS_MAX: u8 = 13;
/// Highest brightness the panel path lights for the Gaming Special Key
/// effect. Brightness 8, 9 and 13 were each sent with this effect at speed 2
/// and left the special-key group dark, while 2, 3, 4 and 5 lit it, so the
/// gaming path is pinned inside 0–5. Other effects light the full record
/// range 0–13 (confirmed 19 October 2026 for the Steady effect), which the
/// Windows app's own UI simply clamps to this value.
pub const WINDOWS_BRIGHTNESS_MAX: u8 = 5;
/// Highest speed the recovered mapping accepts.
pub const SPEED_MAX: u8 = 2;
/// Highest gaming loader byte the recovered mapping accepts.
///
/// The Windows tool exposed three (M1, M2, M3) and its writer maps the
/// stored values 1, 2 and 3 onto the wire bytes 0, 1 and 2. The keyboard
/// has nine lighting zones of its own (FN+1 … FN+9), but it blinks and falls
/// back when it is sent a larger value, so the wire byte carries exactly
/// these three.
pub const SELECT_MAX: u8 = 2;
/// The gaming loader value this unit's firmware implements.
///
/// Live sweeps (see docs/captures/lighting-zone-capture.md) lit the
/// special-key group only with the third loader byte; the first and second
/// left the board dark at every tested brightness, exactly as the original
/// tool does with its own M1 and M2.
pub const LOADER_APPLIED: u8 = 2;

/// The recovered main-panel effect names, in hardware order.
pub const EFFECT_NAMES: [&str; PANEL_EFFECTS] = [
    "Steady",
    "Gaming Special Key",
    "Breathing",
    "Press and destroy",
    "Neon_stream",
    "Streamer",
    "Flowing light and color",
    "Dripping ripples",
    "Brilliant point",
    "Flash away",
    "Shadow_disappear",
    "Ripples shining",
    "Rich and honored",
    "Marquee effect",
    "Rotating storm",
    "Serpentine horse race",
    "Stars twinkle",
    "Retro_snake",
    "Diagonal transformation",
    "Sine wave",
];

/// The firmware query: SET_FEATURE followed by GET_FEATURE of report 7.
pub const FIRMWARE_REQUEST: [u8; 8] = [0x07, 0xF1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

/// The recovered Windows startup record, replayed by the startup probe.
/// Brightness 13 renders no light on this firmware (8 and 9 do not either);
/// the tool sends it as a sync/no-op that leaves the keyboard's own
/// start-up lighting untouched.
pub const STARTUP_RECORD: [u8; 8] = [0x07, 0xFF, 0xFF, 0x00, 0x0D, 0x00, 0x00, 0x00];

/// Builds the seven payload bytes of a main-panel command.
///
/// The gaming path is pinned inside the values the firmware lights: the
/// special-key lighting reacts to selected loader values and hidden speed
/// only inside this combination, and the tool's own gaming page ships the
/// same fixed speed. Other effects keep the tool's baseline loader byte 0,
/// the byte its start-up record carries.
pub fn panel_payload(state: &State) -> [u8; 7] {
    let brightness = state.brightness.min(WINDOWS_BRIGHTNESS_MAX);
    if state.effect == 1 {
        [
            0xFF,
            0xFF,
            0x01,
            brightness,
            SPEED_MAX,
            LOADER_APPLIED,
            0x00,
        ]
    } else {
        [
            0xFF,
            0xFF,
            state.effect as u8,
            brightness,
            state.speed,
            0x00,
            0x00,
        ]
    }
}

/// Renders bytes as spaced uppercase hex, the format used in every report.
pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The capability report. Unsupported paths stay explicitly unverified.
pub fn capabilities_json() -> String {
    format!(
        "{{\"panel_effects\":{},\"panel_brightness_max\":{},\"panel_brightness_max_windows\":{},\"speed_max\":{},\"select_max\":{},\"lighting_control\":\"physically_confirmed\",\"firmware_response\":\"supported\",\"side_control\":\"unsupported\",\"custom_rgb\":\"unverified\",\"per_key_rgb\":\"unverified\"}}",
        PANEL_EFFECTS, BRIGHTNESS_MAX, WINDOWS_BRIGHTNESS_MAX, SPEED_MAX, SELECT_MAX
    )
}

/// The effect catalog, with the hardware limits attached.
pub fn catalog_json() -> String {
    let panel = EFFECT_NAMES
        .iter()
        .map(|n| format!("\"{}\"", n))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"panel\":[{}],\"panel_brightness_max\":{},\"speed_max\":{},\"select_max\":{}}}",
        panel, BRIGHTNESS_MAX, SPEED_MAX, SELECT_MAX
    )
}
