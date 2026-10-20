//! Argument helpers and the usage text.

/// Reads a numeric option value, for example `--brightness 5`.
pub fn opt_u8(args: &[String], name: &str) -> Result<Option<u8>, String> {
    if let Some(i) = args.iter().position(|a| a == name) {
        let v = args
            .get(i + 1)
            .ok_or_else(|| format!("missing value for {}", name))?;
        v.parse::<u8>()
            .map(Some)
            .map_err(|_| format!("{}: invalid number: {}", name, v))
    } else {
        Ok(None)
    }
}

/// Whether a boolean flag is present.
pub fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// Reads an `on`/`off` switch value.
pub fn opt_switch(args: &[String], name: &str) -> Result<Option<bool>, String> {
    if let Some(index) = args.iter().position(|arg| arg == name) {
        match args.get(index + 1).map(String::as_str) {
            Some("on") => Ok(Some(true)),
            Some("off") => Ok(Some(false)),
            Some(value) => Err(format!("{} must be on or off, got {}", name, value)),
            None => Err(format!("missing value for {}", name)),
        }
    } else {
        Ok(None)
    }
}

/// The origin recorded for HID commands: `panel`, `signal` or `cli`.
pub fn command_origin(args: &[String]) -> Result<&str, String> {
    if let Some(index) = args.iter().position(|arg| arg == "--origin") {
        match args.get(index + 1).map(String::as_str) {
            Some("panel") => Ok("panel"),
            Some("signal") => Ok("signal"),
            Some(value) => Err(format!(
                "invalid --origin {}; expected panel or signal",
                value
            )),
            None => Err("missing value for --origin".into()),
        }
    } else {
        Ok("cli")
    }
}

/// The value of `--device`, if given.
pub fn device_arg(args: &[String]) -> Option<&str> {
    args.iter()
        .position(|a| a == "--device")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
}

/// Prints the command overview.
pub fn usage() {
    println!(
        "lecoo-gk301 — OmaLecoo: Lecoo GK301 keyboard RGB control for Omarchy\n\
\n\
Usage:\n\
  lecoo-gk301 capabilities [--json]        show verified capabilities\n\
  lecoo-gk301 doctor                       connection and debug information\n\
  lecoo-gk301 response [--json]            validated firmware return path\n\
  lecoo-gk301 restore [--factory]           apply saved manual settings (or the factory defaults)\n\
  lecoo-gk301 dev [--json]                 show device presence and writability\n\
  lecoo-gk301 status [--json]              show saved manual state (not lighting readback)\n\
  lecoo-gk301 rules [--json]               show automation settings\n\
  lecoo-gk301 rules set --automation <on|off> --restore-on-reconnect <on|off>\n\
  lecoo-gk301 blackbox --json              show recent HID command history\n\
  lecoo-gk301 state clear                  remove saved OmaLecoo state\n\
  lecoo-gk301 udev install|remove          install or remove the device rule (root)\n\
  lecoo-gk301 next | prev                  change panel effect\n\
  lecoo-gk301 panel <0-19> [--brightness 0-13] [--speed 0-2] [--select 0-2]\n\
  lecoo-gk301 brightness <0-13>            panel brightness (the wire lights 0-5)\n\
  lecoo-gk301 preset <name>                silent|game|night|party|off|random\n\
  lecoo-gk301 catalog [--json]             effect catalog\n\
  lecoo-gk301 info [--json]                firmware version\n\
  lecoo-gk301 get                          read feature report 7\n\
  lecoo-gk301 list                         effect names\n\
  lecoo-gk301 macro list|save|remove|play|import|export   lighting macros\n\
  lecoo-gk301 keys list|set|export         GK301 function keys and Hyprland binds\n\
  lecoo-gk301 zones list [--json]           the keyboard's own FN lighting zones\n\
\n\
Options: --device /dev/hidrawN (or LECOO_DEVICE)\n\
         --origin <panel|signal> for HID commands (optional; default: cli)\n\
Firmware query: 07 F1…, panel commands: 07 FF FF … (8-byte Feature reports)\n\
Not built in: side LEDs (no effect on this keyboard), custom RGB, on-device\n\
macros — see docs/FINAL-ANALYSIS.md."
    );
}
