//! OmaLecoo — native Lecoo GK301 (SEMICO `1a2c:9df4`) keyboard RGB control.
//!
//! The crate is split into focused modules so that every responsibility has
//! one owner:
//!
//! | Module | Responsibility |
//! | --- | --- |
//! | [`protocol`] | Wire constants, effect catalog, capability and catalog reports |
//! | [`transport`] | HID feature-report transport and the trait that allows substituting a device |
//! | [`device`] | Device discovery, open/write paths, firmware query |
//! | [`state`] | Saved manual lighting state and automation rules |
//! | [`blackbox`] | Bounded HID command history (`blackbox.jsonl`) |
//! | [`logging`] | Debug log, JSON escaping, UTC timestamps |
//! | [`lock`] | Cross-process command lock shared by every writer |
//! | [`udev`] | The single owned udev rule and its install/remove path |
//! | [`macros`] | Named lighting sequences and their playback |
//! | [`keys`] | The GK301 function keys and the Hyprland fragment for them |
//! | [`cli`] | Argument parsing helpers and the usage text |
//! | [`commands`] | Command implementations behind one dispatch function |
//! | [`probe`] | Hidden research diagnostics (brightness sweep, startup replay) |
//!
//! Protocol (vendor HID interface 1, report ID 7, 8-byte Feature reports):
//!   panel effect : 07 FF FF <effect 0-19> <brightness 0-13> <speed 0-2> <select 0-2> 00
//!   firmware     : 07 F1 00 00 00 00 00 00 (set, then get)
//!
//! Side-lighting, raw and reset paths were removed from this tool: the
//! recovered side writer belongs to the Windows tool's hidden Mouse page and
//! never produced a visible change on the GK301. Live capture and REA
//! analysis live in `docs/FINAL-ANALYSIS.md`.

pub mod blackbox;
pub mod cli;
pub mod commands;
pub mod device;
pub mod keys;
pub mod lock;
pub mod logging;
pub mod macros;
pub mod probe;
pub mod protocol;
pub mod state;
pub mod sys;
pub mod transport;
pub mod udev;
