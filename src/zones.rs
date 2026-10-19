//! The GK301's hardware lighting zones.
//!
//! The keyboard keeps ten lighting layouts in its own controller and its
//! FN+1 … FN+0 keys pick one of them: the manufacturer's GK301 guide lists
//! them as FPS 1, FPS 2, COD, RTS, LOL, Racing, NBA, LOL II, Custom I and
//! Custom II.
//!
//! These zones are **not addressable from the host**. The recovered report
//! (`07 FF | effect | brightness | speed | loader | 00`) has no field for
//! them, the original Windows tool does not send one either — its
//! `HidDevice::SetOutputReport` method exists in the binary but is never
//! called — and the keyboard keeps the chosen zone across host commands.
//! Re-applying the *Gaming Special Key* effect lights whichever zone the
//! keyboard last selected.
//!
//! This module documents what the keyboard offers. It is a list, not a set
//! of commands: nothing here writes to the device.

/// One hardware lighting zone.
pub struct Zone {
    /// The key combination that selects the zone.
    pub key: &'static str,
    /// The name the manufacturer's guide gives the layout.
    pub name: &'static str,
    /// What the zone lights, as observed on the unit.
    pub lights: &'static str,
}

/// The ten zones of the GK301, in key order.
pub const ZONES: [Zone; 10] = [
    Zone {
        key: "FN+1",
        name: "FPS 1",
        lights: "ESC, WASD and the arrow keys",
    },
    Zone {
        key: "FN+2",
        name: "FPS 2",
        lights: "TAB, SHIFT, CTRL, QWER, HOME, PRTSC, SCRLK, PGUP, END, PGDN",
    },
    Zone {
        key: "FN+3",
        name: "COD",
        lights: "observed on the unit",
    },
    Zone {
        key: "FN+4",
        name: "RTS",
        lights: "observed on the unit",
    },
    Zone {
        key: "FN+5",
        name: "LOL",
        lights: "observed on the unit",
    },
    Zone {
        key: "FN+6",
        name: "Racing",
        lights: "observed on the unit",
    },
    Zone {
        key: "FN+7",
        name: "NBA",
        lights: "observed on the unit",
    },
    Zone {
        key: "FN+8",
        name: "LOL II",
        lights: "observed on the unit",
    },
    Zone {
        key: "FN+9",
        name: "Custom I",
        lights: "observed on the unit",
    },
    Zone {
        key: "FN+0",
        name: "Custom II",
        lights: "observed on the unit",
    },
];

/// The zones, in key order.
pub fn list() -> &'static [Zone] {
    &ZONES
}

/// The zones as a JSON document.
pub fn list_json() -> String {
    let items = ZONES
        .iter()
        .map(|zone| {
            format!(
                "{{\"key\":\"{}\",\"name\":\"{}\",\"lights\":\"{}\"}}",
                zone.key, zone.name, zone.lights
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"zones\":[{}]}}", items)
}
