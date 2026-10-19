//! Lecoo GK301 (SEMICO 1a2c:9df4) keyboard RGB control.
//!
//! Protocol (vendor HID interface 1, report ID 7, 8-byte Feature reports):
//!   panel effect : 07 FF FF <effect 0-19> <brightness 0-13> <speed 0-2> <select 0-2> 00
//!   firmware     : 07 F1 00 00 00 00 00 00 (set, then get)
//!
//! Side-lighting, raw and reset paths were removed from this tool: the
//! recovered side writer belongs to the Windows tool's hidden Mouse page and
//! never produced a visible change on the GK301. Live capture and REA
//! analysis live in docs/FINAL-ANALYSIS.md.

use std::env;
use std::ffi::{c_int, c_ulong, CString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::thread;
use std::time::Duration;

const REPORT_ID: u8 = 0x07;
const VID: u32 = 0x1a2c;
const PID: u32 = 0x9df4;
const PANEL_EFFECTS: usize = 20;

const O_NONBLOCK: i32 = 0o4000;

const EFFECT_NAMES: [&str; PANEL_EFFECTS] = [
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

const IOC_WRITE: c_ulong = 1;
const IOC_READ: c_ulong = 2;

extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn flock(fd: c_int, operation: c_int) -> c_int;
    fn signal(signum: c_int, handler: usize) -> usize;
    fn access(path: *const i8, mode: c_int) -> c_int;
    fn geteuid() -> u32;
}

const SIGPIPE: c_int = 13;
const BLACKBOX_LIMIT: usize = 1_048_576;
const UDEV_RULE_PATH: &str = "/etc/udev/rules.d/70-lecoo-gk301.rules";
const UDEV_RULE_CONTENT: &str = "# Lecoo GK301 (1a2c:9df4) vendor HID interface (report ID 7)\nSUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"1a2c\", ATTRS{idProduct}==\"9df4\", ENV{ID_USB_INTERFACE_NUM}==\"01\", TAG+=\"uaccess\"\n";

fn ioc(dir: c_ulong, ty: u8, nr: u8, size: usize) -> c_ulong {
    (dir << 30) | ((size as c_ulong) << 16) | ((ty as c_ulong) << 8) | nr as c_ulong
}

fn hid_set_feature(size: usize) -> c_ulong {
    ioc(IOC_WRITE | IOC_READ, b'H', 0x06, size)
}

fn hid_get_feature(size: usize) -> c_ulong {
    ioc(IOC_WRITE | IOC_READ, b'H', 0x07, size)
}

fn bounded_value(value: u64, max: u8) -> u8 {
    value.min(max as u64) as u8
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone)]
struct State {
    effect: usize,
    brightness: u8,
    speed: u8,
    select: u8,
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
    fn path() -> PathBuf {
        if let Ok(dir) = env::var("XDG_STATE_HOME") {
            PathBuf::from(dir).join("lecoo-gk301").join("state.json")
        } else {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home)
                .join(".local/state/lecoo-gk301")
                .join("state.json")
        }
    }

    fn load() -> Self {
        let text = match fs::read_to_string(Self::path()) {
            Ok(t) => t,
            Err(_) => return Self::default(),
        };
        let mut st = Self::default();
        if let Some(v) = find_u64(&text, "effect") {
            st.effect = (v as usize) % PANEL_EFFECTS;
        }
        if let Some(v) = find_u64(&text, "brightness") {
            st.brightness = bounded_value(v, 13);
        }
        if let Some(v) = find_u64(&text, "speed") {
            st.speed = bounded_value(v, 2);
        }
        if let Some(v) = find_u64(&text, "select") {
            st.select = bounded_value(v, 2);
        }
        st
    }

    fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
            .map_err(|e| format!("device command sent but state could not be saved: {}", e))
    }

    fn save_to(&self, path: &std::path::Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, format!("{}\n", self.json()))?;
        fs::rename(&tmp, path)
    }

    fn name(&self) -> &'static str {
        EFFECT_NAMES[self.effect]
    }

    fn json(&self) -> String {
        format!(
            "{{\"effect\":{},\"name\":\"{}\",\"brightness\":{},\"speed\":{},\"select\":{},\"panel_count\":{},\"panel_brightness_max\":13,\"speed_max\":2,\"select_max\":2}}",
            self.effect,
            self.name(),
            self.brightness,
            self.speed,
            self.select,
            PANEL_EFFECTS
        )
    }

    fn validate(&self) -> Result<(), String> {
        if self.effect >= PANEL_EFFECTS {
            return Err(format!("panel effect must be in 0..{}", PANEL_EFFECTS - 1));
        }
        if self.brightness > 13 {
            return Err("panel brightness must be in 0..13".into());
        }
        if self.speed > 2 {
            return Err("speed must be in 0..2".into());
        }
        if self.select > 2 {
            return Err("selection must be in 0..2".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct Rules {
    automation_enabled: bool,
    restore_on_reconnect: bool,
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
    fn path() -> PathBuf {
        State::path().with_file_name("rules.json")
    }

    fn load() -> Self {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    fn parse(text: &str) -> Option<Self> {
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

    fn json(self) -> String {
        format!(
            "{{\"automation_enabled\":{},\"restore_on_reconnect\":{}}}",
            self.automation_enabled, self.restore_on_reconnect
        )
    }

    fn save(self) -> io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, format!("{}\n", self.json()))?;
        fs::rename(tmp, path)
    }
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

struct Device {
    file: File,
    path: PathBuf,
}

impl Device {
    fn open(path: PathBuf) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("{}: {} ({})", path.display(), e, permission_hint(&e)))?;
        debug_log(&format!("device_open path={}", path.display()));
        Ok(Self { file, path })
    }

    fn open_detected(explicit: Option<&str>) -> Result<Self, String> {
        if let Some(p) = explicit {
            return Self::open(PathBuf::from(p));
        }
        if let Ok(p) = env::var("LECOO_DEVICE") {
            if !p.is_empty() {
                return Self::open(PathBuf::from(p));
            }
        }
        match detect_device() {
            Some(path) => Self::open(path),
            None => Err(
                "keyboard not found (1a2c:9df4, interface 1). Check USB or specify --device."
                    .into(),
            ),
        }
    }

    fn set_feature(&self, payload: [u8; 7], origin: &str) -> io::Result<()> {
        let mut buf = [0u8; 8];
        buf[0] = REPORT_ID;
        buf[1..].copy_from_slice(&payload);
        let rc = unsafe {
            ioctl(
                self.file.as_raw_fd(),
                hid_set_feature(buf.len()),
                buf.as_ptr(),
            )
        };
        let error = if rc < 0 {
            Some(io::Error::last_os_error())
        } else {
            None
        };
        debug_log(&format!(
            "hid ioctl rc={} report={} error={}",
            rc,
            hex(&buf),
            error.as_ref().map(ToString::to_string).unwrap_or_default()
        ));
        record_blackbox(origin, &buf, rc, error.as_ref(), "feature report");
        if let Some(error) = error {
            Err(error)
        } else {
            Ok(())
        }
    }

    fn get_feature(&self, origin: &str) -> io::Result<[u8; 8]> {
        let mut buf = [0u8; 8];
        buf[0] = REPORT_ID;
        let rc = unsafe {
            ioctl(
                self.file.as_raw_fd(),
                hid_get_feature(buf.len()),
                buf.as_mut_ptr(),
            )
        };
        let error = if rc < 0 {
            Some(io::Error::last_os_error())
        } else {
            None
        };
        debug_log(&format!(
            "hid ioctl rc={} report={} error={}",
            rc,
            hex(&buf),
            error.as_ref().map(ToString::to_string).unwrap_or_default()
        ));
        record_blackbox(origin, &buf, rc, error.as_ref(), "firmware acknowledgement");
        if let Some(error) = error {
            Err(error)
        } else {
            Ok(buf)
        }
    }
}

fn permission_hint(e: &io::Error) -> String {
    if e.raw_os_error() == Some(13) {
        "permission denied: udev rule is not installed".to_string()
    } else {
        e.to_string()
    }
}

fn detect_device() -> Option<PathBuf> {
    let mut entries: Vec<_> = fs::read_dir("/sys/class/hidraw")
        .ok()?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("hidraw") {
            continue;
        }
        let uevent = match fs::read_to_string(entry.path().join("device/uevent")) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let mut hid_id = None;
        let mut phys = None;
        for line in uevent.lines() {
            if let Some(v) = line.strip_prefix("HID_ID=") {
                hid_id = Some(v.trim().to_ascii_uppercase());
            } else if let Some(v) = line.strip_prefix("HID_PHYS=") {
                phys = Some(v.trim().to_string());
            }
        }
        let Some(id) = hid_id else { continue };
        let parts: Vec<&str> = id.split(':').collect();
        if parts.len() != 3 {
            continue;
        }
        let parse = |s: &str| u32::from_str_radix(s, 16).ok();
        let (Some(_bus), Some(vid), Some(pid)) =
            (parse(parts[0]), parse(parts[1]), parse(parts[2]))
        else {
            continue;
        };
        if vid != VID || pid != PID {
            continue;
        }
        if !phys.as_deref().unwrap_or("").ends_with("/input1") {
            continue;
        }
        return Some(PathBuf::from(format!("/dev/{}", name)));
    }

    let by_id = PathBuf::from("/dev/input/by-id/usb-SEMICO_USB_Gaming_Keyboard-if01-hidraw");
    if by_id.exists() {
        return Some(by_id);
    }
    None
}

fn send_panel(dev: &Device, st: &State, origin: &str) -> Result<(), String> {
    st.validate()?;
    dev.set_feature(
        [
            0xFF,
            0xFF,
            st.effect as u8,
            st.brightness,
            st.speed,
            st.select,
            0x00,
        ],
        origin,
    )
    .map_err(|e| {
        format!(
            "could not send panel command: {} ({})",
            e,
            permission_hint(&e)
        )
    })
}

const FIRMWARE_REQUEST: [u8; 8] = [0x07, 0xF1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

/// Validated firmware acknowledgment, as recoverable from the Windows tool's
/// startup query (set 07 F1…, then get report 7). This is the device's reply,
/// not a lighting-state readback: live capture confirmed the readback carries
/// firmware bytes even right after a lighting change.
struct FirmwareResponse {
    device: PathBuf,
    raw: [u8; 8],
    firmware: String,
    roundtrip_ms: u128,
}

/// Feature transfer whose byte count is validated: HIDIOCSFEATURE/HIDIOCGFEATURE
/// must return exactly the report length, never a short result.
fn transfer_exact(dev: &Device, send: bool, report: *mut u8, origin: &str) -> io::Result<()> {
    let request = if send {
        hid_set_feature(FIRMWARE_REQUEST.len())
    } else {
        hid_get_feature(FIRMWARE_REQUEST.len())
    };
    let rc = unsafe { ioctl(dev.file.as_raw_fd(), request, report) };
    let error = if rc < 0 {
        Some(io::Error::last_os_error())
    } else if rc as usize != FIRMWARE_REQUEST.len() {
        Some(io::Error::other(format!(
            "short transfer ({rc} bytes, expected 8)"
        )))
    } else {
        None
    };
    let payload = unsafe { std::slice::from_raw_parts(report.cast_const(), 8) };
    debug_log(&format!(
        "hid {} rc={} report={} error={}",
        if send { "set" } else { "get" },
        rc,
        hex(payload),
        error.as_ref().map(ToString::to_string).unwrap_or_default()
    ));
    let report: [u8; 8] = payload.try_into().expect("fixed-size HID report");
    record_blackbox(
        origin,
        &report,
        rc,
        error.as_ref(),
        if send {
            "firmware query"
        } else {
            "firmware acknowledgement"
        },
    );
    if let Some(error) = error {
        Err(error)
    } else {
        Ok(())
    }
}

fn firmware_string(raw: &[u8; 8]) -> Result<String, String> {
    if raw[0] != REPORT_ID {
        return Err(format!(
            "unexpected response report id {:02X} (expected {:02X})",
            raw[0], REPORT_ID
        ));
    }
    let major = raw[1];
    let high = (raw[2] >> 4) & 0x0F;
    let low = raw[2] & 0x0F;
    if major == 0 || major > 9 || high > 9 || low > 9 {
        return Err(format!(
            "unexpected firmware bytes {:02X} {:02X}",
            raw[1], raw[2]
        ));
    }
    Ok(format!("{}.{:X}{:X}", major, high, low))
}

fn query_firmware(dev: &Device, origin: &str) -> Result<FirmwareResponse, String> {
    let start = std::time::Instant::now();
    let mut request = FIRMWARE_REQUEST;
    transfer_exact(dev, true, request.as_mut_ptr(), origin).map_err(|e| {
        format!(
            "could not send firmware query: {} ({})",
            e,
            permission_hint(&e)
        )
    })?;
    thread::sleep(Duration::from_millis(50));
    let mut raw = FIRMWARE_REQUEST;
    transfer_exact(dev, false, raw.as_mut_ptr(), origin)
        .map_err(|e| format!("could not read response: {} ({})", e, permission_hint(&e)))?;
    let firmware = firmware_string(&raw)?;
    Ok(FirmwareResponse {
        device: dev.path.clone(),
        raw,
        firmware,
        roundtrip_ms: start.elapsed().as_millis(),
    })
}

fn print_response(resp: &FirmwareResponse, json: bool) {
    if json {
        println!(
            "{{\"device\":\"{}\",\"request\":\"{}\",\"raw\":\"{}\",\"firmware\":\"{}\",\"kind\":\"firmware\",\"lighting_readback\":false,\"roundtrip_ms\":{}}}",
            resp.device.display(),
            hex(&FIRMWARE_REQUEST),
            hex(&resp.raw),
            resp.firmware,
            resp.roundtrip_ms
        );
    } else {
        println!("device: {}", resp.device.display());
        println!("request: {}", hex(&FIRMWARE_REQUEST));
        println!("response: {}", hex(&resp.raw));
        println!("firmware: {}", resp.firmware);
        println!("roundtrip: {} ms", resp.roundtrip_ms);
        println!("kind: firmware acknowledgment; the readback does not carry lighting state");
    }
}

fn opt_u8(args: &[String], name: &str) -> Result<Option<u8>, String> {
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

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn opt_switch(args: &[String], name: &str) -> Result<Option<bool>, String> {
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

fn capabilities_json() -> String {
    format!(
        "{{\"panel_effects\":{},\"panel_brightness_max\":13,\"panel_brightness_max_windows\":5,\"speed_max\":2,\"select_max\":2,\"lighting_control\":\"physically_confirmed\",\"firmware_response\":\"supported\",\"side_control\":\"unsupported\",\"custom_rgb\":\"unverified\",\"per_key_rgb\":\"unverified\"}}",
        PANEL_EFFECTS
    )
}

fn catalog_json() -> String {
    let panel = EFFECT_NAMES
        .iter()
        .map(|n| format!("\"{}\"", n))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"panel\":[{}],\"panel_brightness_max\":13,\"speed_max\":2,\"select_max\":2}}",
        panel
    )
}

fn random_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ ((std::process::id() as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15
}

fn preset_state(name: &str) -> Result<State, String> {
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

fn usage() {
    println!(
        "lecoo-gk301 — OmaLecoo: Lecoo GK301 keyboard RGB control for Omarchy\n\
\n\
Usage:\n\
  lecoo-gk301 capabilities [--json]        show verified capabilities\n\
  lecoo-gk301 doctor                       connection and debug information\n\
  lecoo-gk301 response [--json]            validated firmware return path\n\
  lecoo-gk301 restore                      apply saved manual settings\n\
  lecoo-gk301 dev [--json]                 show device presence and writability\n\
  lecoo-gk301 status [--json]              show saved manual state (not lighting readback)\n\
  lecoo-gk301 rules [--json]               show automation settings\n\
  lecoo-gk301 rules set --automation <on|off> --restore-on-reconnect <on|off>\n\
  lecoo-gk301 blackbox --json              show recent HID command history\n\
  lecoo-gk301 state clear                  remove saved OmaLecoo state\n\
  lecoo-gk301 udev install|remove          install or remove the device rule (root)\n\
  lecoo-gk301 next | prev                  change panel effect\n\
  lecoo-gk301 panel <0-19> [--brightness 0-13] [--speed 0-2] [--select 0-2]\n\
  lecoo-gk301 brightness <0-13>            panel brightness (Windows app clamps to 5)\n\
  lecoo-gk301 preset <name>                silent|game|night|party|off|random\n\
  lecoo-gk301 catalog [--json]             effect catalog\n\
  lecoo-gk301 info [--json]                firmware version\n\
  lecoo-gk301 get                          read feature report 7\n\
  lecoo-gk301 list                         effect names\n\
\n\
Options: --device /dev/hidrawN (or LECOO_DEVICE)\n\
         --origin <panel|signal> for HID commands (optional; default: cli)\n\
Firmware query: 07 F1…, panel commands: 07 FF FF … (8-byte Feature reports)\n\
Not built in: side LEDs (no effect on this keyboard), custom RGB, on-device\n\
macros — see docs/FINAL-ANALYSIS.md."
    );
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(cmd) = args.first().map(|s| s.as_str()) else {
        usage();
        return Ok(());
    };
    let origin = command_origin(&args)?;
    let device_arg = args
        .iter()
        .position(|a| a == "--device")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());

    let _lock = command_lock()?;
    debug_log(&format!("command={:?}", args));
    // Windows sender enforces a shared 100 ms gate (0x41feb0/0x41ffd0).
    // Delay under the process lock so separate invocations obey it too.
    if !matches!(
        cmd,
        "status"
            | "catalog"
            | "list"
            | "help"
            | "-h"
            | "--help"
            | "dev"
            | "capabilities"
            | "probe-brightness"
            | "rules"
            | "blackbox"
            | "state"
            | "udev"
    ) {
        thread::sleep(Duration::from_millis(110));
    }
    match cmd {
        "capabilities" => println!("{}", capabilities_json()),
        "doctor" => {
            let dev = Device::open_detected(device_arg)?;
            println!("device: {}", dev.path.display());
            println!("state: {}", State::path().display());
            println!(
                "debug: {}",
                State::path().with_file_name("debug.log").display()
            );
            let resp = query_firmware(&dev, origin)?;
            println!("request: {}", hex(&FIRMWARE_REQUEST));
            println!(
                "firmware report: {} (firmware {})",
                hex(&resp.raw),
                resp.firmware
            );
            println!("Firmware response received. This query does not confirm that lighting settings were applied; the readback does not carry lighting state.");
        }
        "restore" => {
            let st = State::load();
            let dev = Device::open_detected(device_arg)?;
            send_panel(&dev, &st, origin)?;
            println!("Last saved lighting settings applied");
        }
        "response" => {
            let dev = Device::open_detected(device_arg)?;
            let resp = query_firmware(&dev, origin)?;
            print_response(&resp, has_flag(&args, "--json") || has_flag(&args, "-j"));
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
            if has_flag(&args, "--json") || has_flag(&args, "-j") {
                println!("{}", catalog_json());
            } else {
                for (i, name) in EFFECT_NAMES.iter().enumerate() {
                    println!("panel {:2}  {}", i, name);
                }
            }
        }
        "dev" => {
            let path = detect_device();
            if has_flag(&args, "--json") {
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
                let automation = opt_switch(&args, "--automation")?;
                let restore = opt_switch(&args, "--restore-on-reconnect")?;
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
            if !has_flag(&args, "--json") {
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
            if has_flag(&args, "--json") {
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
            let dev = Device::open_detected(device_arg)?;
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
            if let Some(v) = opt_u8(&args, "--brightness")? {
                st.brightness = v;
            }
            if let Some(v) = opt_u8(&args, "--speed")? {
                st.speed = v;
            }
            if let Some(v) = opt_u8(&args, "--select")? {
                st.select = v;
            }
            st.validate()?;
            let dev = Device::open_detected(device_arg)?;
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
            let dev = Device::open_detected(device_arg)?;
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
            let dev = Device::open_detected(device_arg)?;
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
            let dev = Device::open_detected(device_arg)?;
            dev.set_feature([0xF1, 0, 0, 0, 0, 0, 0], origin)
                .map_err(|e| format!("could not send query: {} ({})", e, permission_hint(&e)))?;
            thread::sleep(Duration::from_millis(50));
            let resp = dev
                .get_feature(origin)
                .map_err(|e| format!("could not read response: {} ({})", e, permission_hint(&e)))?;
            let major = resp[1];
            let minor = resp[2];
            let firmware = if major <= 9 {
                format!("{}.{:X}{:X}", major, (minor >> 4) & 0x0F, minor & 0x0F)
            } else {
                String::new()
            };
            if has_flag(&args, "--json") || has_flag(&args, "-j") {
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
            let dev = Device::open_detected(device_arg)?;
            let resp = dev
                .get_feature(origin)
                .map_err(|e| format!("could not read: {} ({})", e, permission_hint(&e)))?;
            println!("{}", hex(&resp));
            println!("(this readback carries firmware data, not lighting state)");
        }
        "probe-brightness" => probe_brightness(&args, device_arg, origin)?,
        other => {
            return Err(format!("unknown command: {}\n\nlecoo-gk301 --help", other));
        }
    }
    Ok(())
}

fn command_lock() -> Result<File, String> {
    let path = State::path().with_file_name("command.lock");
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    // Lock across processes: firmware queries, CLI writes and all panel instances.
    if unsafe { flock(file.as_raw_fd(), 2) } < 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    Ok(file)
}

fn debug_log(message: &str) {
    let path = State::path().with_file_name("debug.log");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::metadata(&path)
        .map(|m| m.len() > 1_048_576)
        .unwrap_or(false)
    {
        let _ = fs::rename(&path, path.with_extension("log.1"));
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let _ = writeln!(f, "{} pid={} {}", now, std::process::id(), message);
    }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => escaped.push(' '),
            c => escaped.push(c),
        }
    }
    escaped
}

fn utc_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;
    // Gregorian civil date conversion, with 1970-01-01 as day zero.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        day_seconds / 3_600,
        (day_seconds / 60) % 60,
        day_seconds % 60
    )
}

fn record_blackbox(
    origin: &str,
    report: &[u8; 8],
    rc: c_int,
    error: Option<&io::Error>,
    result: &str,
) {
    let path = State::path().with_file_name("blackbox.jsonl");
    let rotated = State::path().with_file_name("blackbox.jsonl.1");
    let entry = format!(
        "{{\"utc\":\"{}\",\"origin\":\"{}\",\"report\":\"{}\",\"syscall\":{{\"return\":{},\"error\":{}}},\"result\":\"{}\"}}\n",
        utc_timestamp(),
        json_escape(origin),
        hex(report),
        rc,
        error.map(|e| format!("\"{}\"", json_escape(&e.to_string()))).unwrap_or_else(|| "null".into()),
        json_escape(&format!(
            "{} {}",
            result,
            if error.is_some() { "failed" } else { "succeeded" }
        ))
    );
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let current_len = fs::metadata(&path).map(|m| m.len() as usize).unwrap_or(0);
    if current_len > 0 && current_len.saturating_add(entry.len()) > BLACKBOX_LIMIT {
        match fs::remove_file(&rotated) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return,
        }
        if fs::rename(&path, &rotated).is_err() {
            return;
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(entry.as_bytes());
    }
}

fn blackbox_json() -> String {
    let current = State::path().with_file_name("blackbox.jsonl");
    let rotated = State::path().with_file_name("blackbox.jsonl.1");
    let mut entries = Vec::new();
    for path in [&current, &rotated] {
        if let Ok(contents) = fs::read_to_string(path) {
            let mut file_entries: Vec<String> = contents
                .lines()
                .filter(|line| {
                    matches!(
                        serde_json::from_str::<serde_json::Value>(line),
                        Ok(serde_json::Value::Object(_))
                    )
                })
                .map(str::to_owned)
                .collect();
            file_entries.reverse();
            entries.extend(file_entries);
        }
    }
    format!("[{}]", entries.join(","))
}

fn command_origin(args: &[String]) -> Result<&str, String> {
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

fn device_presence_json() -> String {
    match detect_device() {
        Some(path) => {
            format!(
                "{{\"connected\":true,\"path\":\"{}\",\"writable\":{}}}",
                json_escape(&path.display().to_string()),
                path_is_writable(&path)
            )
        }
        None => "{\"connected\":false,\"path\":null,\"writable\":false}".into(),
    }
}

fn path_is_writable(path: &std::path::Path) -> bool {
    CString::new(path.as_os_str().as_encoded_bytes())
        .map(|path| unsafe { access(path.as_ptr(), 2) == 0 })
        .unwrap_or(false)
}

fn clear_state() -> Result<(), String> {
    let state_path = State::path();
    let dir = state_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
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
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("could not remove {}: {}", path.display(), error)),
        }
    }
    println!("OmaLecoo state cleared");
    Ok(())
}

fn run_udevadm(args: &[&str]) -> Result<(), String> {
    let status = Command::new("udevadm")
        .args(args)
        .status()
        .map_err(|e| format!("could not run udevadm: {}", e))?;
    if !status.success() {
        return Err(format!("udevadm {} failed with {}", args.join(" "), status));
    }
    Ok(())
}

fn udev_command(action: &str) -> Result<(), String> {
    if unsafe { geteuid() } != 0 {
        return Err("udev install/remove must be run as root (for example through pkexec)".into());
    }
    let path = std::path::Path::new(UDEV_RULE_PATH);
    match action {
        "install" => match fs::read_to_string(path) {
            Ok(existing) if existing == UDEV_RULE_CONTENT => {}
            Ok(_) => {
                return Err(format!(
                    "refusing to overwrite {} because its contents differ",
                    UDEV_RULE_PATH
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::write(path, UDEV_RULE_CONTENT)
                    .map_err(|e| format!("could not install {}: {}", UDEV_RULE_PATH, e))?;
            }
            Err(error) => return Err(format!("could not read {}: {}", UDEV_RULE_PATH, error)),
        },
        "remove" => match fs::read_to_string(path) {
            Ok(existing) if existing == UDEV_RULE_CONTENT => fs::remove_file(path)
                .map_err(|e| format!("could not remove {}: {}", UDEV_RULE_PATH, e))?,
            Ok(_) => {
                return Err(format!(
                    "refusing to remove {} because its contents differ",
                    UDEV_RULE_PATH
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("could not read {}: {}", UDEV_RULE_PATH, error)),
        },
        _ => return Err("usage: lecoo-gk301 udev install|remove".into()),
    }
    run_udevadm(&["control", "--reload-rules"])?;
    run_udevadm(&["trigger", "--action=add", "--subsystem-match=hidraw"])?;
    run_udevadm(&["settle"])?;
    println!("udev rule {} complete", action);
    Ok(())
}

fn main() -> ExitCode {
    // Rust installs SIG_IGN for SIGPIPE, which turns `lecoo-gk301 list | head`
    // into a panic (and an abort + core dump with panic=abort). Restore the
    // traditional pipe behaviour: exit quietly when the reader goes away.
    unsafe {
        signal(SIGPIPE, 0);
    }
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            debug_log(&format!("ERROR {}", e));
            eprintln!("lecoo-gk301: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Hidden research diagnostic (not listed in usage): replays the Windows
/// tool's startup-form sweep — the stored-record write with an unclamped
/// brightness byte — on the live keyboard, and records any vendor input
/// reports from a concurrent non-blocking reader. Results are written to
/// a JSON file; see docs/captures/extended-brightness-experiment.md.
fn probe_brightness(args: &[String], device_arg: Option<&str>, origin: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    let flag = |name: &str, fallback: &str| -> String {
        if let Some(i) = args.iter().position(|a| a == name) {
            args.get(i + 1)
                .cloned()
                .unwrap_or_else(|| fallback.to_string())
        } else {
            fallback.to_string()
        }
    };
    let out_path = if let Some(i) = args.iter().position(|a| a == "--out") {
        PathBuf::from(args.get(i + 1).ok_or("missing value for --out")?)
    } else {
        return Err("probe-brightness requires --out <path>".into());
    };
    let wait_first: f64 = flag("--wait-first", "0")
        .parse()
        .map_err(|_| "--wait-first must be a number of seconds".to_string())?;
    let sweep = {
        let mut list = Vec::new();
        for part in flag("--brightnesses", "8,10,13").split(',') {
            if part.trim().is_empty() {
                continue;
            }
            let v: u8 = part
                .trim()
                .parse()
                .map_err(|_| format!("invalid brightness value: {part}"))?;
            if v > 13 {
                return Err("probe brightness must be in 0..13".into());
            }
            list.push(v);
        }
        if list.is_empty() {
            return Err("--brightnesses needs at least one value".into());
        }
        list
    };

    let device = Device::open_detected(device_arg)?;
    let mut read_opts = OpenOptions::new();
    read_opts.read(true).custom_flags(O_NONBLOCK);
    let mut reader_fd = read_opts
        .open(&device.path)
        .map_err(|e| format!("could not open read handle: {e}"))?;
    let t0 = std::time::Instant::now();
    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let record = |events: &Arc<Mutex<Vec<String>>>, kind: &str, extra: &str| {
        let line = format!(
            "{{\"t\":{:.3},\"kind\":\"{}\"{}}}",
            t0.elapsed().as_secs_f64(),
            kind,
            extra
        );
        println!("{line}");
        let mut stored = events.lock().unwrap_or_else(|p| p.into_inner());
        stored.push(line);
    };

    let stop = Arc::new(AtomicBool::new(false));
    let reader_stop = Arc::clone(&stop);
    let reader_events = Arc::clone(&events);
    let reader = thread::spawn(move || {
        let log = |kind: &str, extra: &str| -> String {
            let line = format!(
                "{{\"t\":{:.3},\"kind\":\"{}\"{}}}",
                t0.elapsed().as_secs_f64(),
                kind,
                extra
            );
            println!("{line}");
            line
        };
        let mut seen: Vec<String> = Vec::new();
        let mut buf = [0u8; 64];
        loop {
            if reader_stop.load(Ordering::Relaxed) {
                break;
            }
            match reader_fd.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => {
                    let raw = hex(&buf[..n]);
                    if !seen.contains(&raw) {
                        seen.push(raw.clone());
                        let line = log(
                            "input_report",
                            &format!(",\"report_id\":{},\"raw\":\"{}\"", buf[0], raw),
                        );
                        reader_events
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .push(line);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    let line = log(
                        "reader_error",
                        &format!(",\"error\":\"{}\"", e.to_string().replace('"', "\\\"")),
                    );
                    reader_events
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .push(line);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
    });

    if wait_first > 0.0 {
        let until = t0 + Duration::from_secs_f64(wait_first);
        while std::time::Instant::now() < until {
            thread::sleep(Duration::from_millis(25));
        }
    }

    record(
        &events,
        "start",
        &format!(",\"device\":\"{}\"", device.path.display()),
    );

    let mut step_error: Option<String> = None;
    // 1) Firmware acknowledgment (known-good sanity path).
    match query_firmware(&device, origin) {
        Ok(resp) => record(
            &events,
            "firmware_response",
            &format!(
                ",\"raw\":\"{}\",\"firmware\":\"{}\"",
                hex(&resp.raw),
                resp.firmware
            ),
        ),
        Err(e) => step_error = Some(format!("firmware query failed: {e}")),
    }
    if step_error.is_none() {
        thread::sleep(Duration::from_secs(1));
        // 2) Baseline: the clamped panel write, Steady at 5.
        let payload: [u8; 8] = [REPORT_ID, 0xFF, 0xFF, 0x00, 0x05, 0x02, 0x00, 0x00];
        record(
            &events,
            "set_feature",
            &format!(",\"raw\":\"{}\"", hex(&payload)),
        );
        match device.set_feature([0xFF, 0xFF, 0x00, 0x05, 0x02, 0x00, 0x00], origin) {
            Ok(()) => record(
                &events,
                "set_feature_done",
                &format!(",\"raw\":\"{}\"", hex(&payload)),
            ),
            Err(e) => {
                step_error = Some(format!(
                    "baseline write failed: {} ({})",
                    e,
                    permission_hint(&e)
                ))
            }
        }
    }
    // 3) Startup-form writes: stored-record path (unclamped brightness byte).
    if step_error.is_none() {
        thread::sleep(Duration::from_millis(1200));
        for b in sweep {
            let payload: [u8; 8] = [REPORT_ID, 0xFF, 0xFF, 0x00, b, 0x00, 0x00, 0x00];
            record(
                &events,
                "set_feature",
                &format!(",\"raw\":\"{}\"", hex(&payload)),
            );
            match device.set_feature([0xFF, 0xFF, 0x00, b, 0x00, 0x00, 0x00], origin) {
                Ok(()) => record(
                    &events,
                    "set_feature_done",
                    &format!(",\"raw\":\"{}\"", hex(&payload)),
                ),
                Err(e) => {
                    step_error = Some(format!(
                        "brightness {b} write failed: {} ({})",
                        e,
                        permission_hint(&e)
                    ));
                    break;
                }
            }
            thread::sleep(Duration::from_millis(1200));
        }
    }
    // 4) Restore the user's own last settings through the normal writer.
    if step_error.is_none() {
        let st = State::load();
        record(
            &events,
            "restored",
            &format!(",\"state\":\"{}\"", st.json().replace('"', "\\\"")),
        );
        if let Err(e) = send_panel(&device, &st, origin) {
            step_error = Some(format!("restore write failed: {e}"));
        }
    }

    thread::sleep(Duration::from_millis(500));
    stop.store(true, Ordering::Relaxed);
    let _ = reader.join();
    if let Some(err) = step_error {
        return Err(err);
    }
    let stored = events.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let report = format!(
        "{{\n  \"device\": \"{}\",\n  \"events\": [\n    {}\n  ]\n}}\n",
        device.path.display(),
        stored.join(",\n    ")
    );
    fs::write(&out_path, report)
        .map_err(|e| format!("could not write {}: {e}", out_path.display()))?;
    println!("saved {} events -> {}", stored.len(), out_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capabilities_does_not_claim_unverified_support() {
        let report = capabilities_json();
        assert!(report.contains("\"custom_rgb\":\"unverified\""));
        assert!(report.contains("\"per_key_rgb\":\"unverified\""));
        assert!(report.contains("\"side_control\":\"unsupported\""));
        assert!(report.contains("\"physically_confirmed\""));
        assert!(!report.contains("fixed_side_palette"));
    }

    #[test]
    fn firmware_string_rejects_impossible_versions() {
        assert_eq!(
            firmware_string(&[0x07, 0x01, 0x16, 0, 0, 0, 0, 0]).as_deref(),
            Ok("1.16")
        );
        assert!(firmware_string(&[0x09, 0x01, 0x16, 0, 0, 0, 0, 0]).is_err());
        assert!(firmware_string(&[0x07, 0x00, 0x00, 0, 0, 0, 0, 0]).is_err());
        assert!(firmware_string(&[0x07, 0x01, 0xFA, 0, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn cached_large_values_clamp_before_narrowing() {
        assert_eq!(bounded_value(256, 5), 5);
        assert_eq!(bounded_value(u64::MAX, 2), 2);
        assert_eq!(bounded_value(0, 5), 0);
    }
    #[test]
    fn state_save_reports_failure() {
        let path = PathBuf::from("/proc/lecoo-test-impossible/state.json");
        assert!(State::default().save_to(&path).is_err());
    }
}
