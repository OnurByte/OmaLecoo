//! Lecoo GK301 (SEMICO 1a2c:9df4) keyboard RGB control.
//!
//! Protocol (vendor HID interface 1, report ID 7, 8-byte Feature reports):
//!   panel effect : 07 FF FF <effect 0-19> <brightness 0-5> <speed 0-2> <select 0-2> 00
//!   firmware     : 07 F1 00 00 00 00 00 00 (set, then get)
//!
//! Side-lighting, raw and reset paths were removed from this tool: the
//! recovered side writer belongs to the Windows tool's hidden Mouse page and
//! never produced a visible change on the GK301. Live capture and REA
//! analysis live in docs/FINAL-ANALYSIS.md.

use std::env;
use std::ffi::{c_int, c_ulong};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

const REPORT_ID: u8 = 0x07;
const VID: u32 = 0x1a2c;
const PID: u32 = 0x9df4;
const PANEL_EFFECTS: usize = 20;

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
}

const SIGPIPE: c_int = 13;

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

    fn load_from(path: &std::path::Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("could not read profile: {e}"))?;
        let body = text
            .trim()
            .strip_prefix('{')
            .and_then(|t| t.strip_suffix('}'))
            .ok_or("invalid profile JSON")?;
        let mut fields = std::collections::HashMap::new();
        for entry in body.split(',') {
            let (key, value) = entry.split_once(':').ok_or("invalid profile field")?;
            let key = key
                .trim()
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .filter(|s| !s.contains('"'))
                .ok_or("invalid profile field name")?;
            let value = value.trim();
            if value.is_empty() || fields.insert(key, value).is_some() {
                return Err("missing or duplicate profile field".into());
            }
            if matches!(key, "name" | "side_name") {
                let label = value
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .ok_or("invalid profile label")?;
                if label.contains(['"', '\\']) || label.chars().any(char::is_control) {
                    return Err("invalid profile label".into());
                }
            } else if !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err("profile values must be nonnegative integers".into());
            }
        }
        let number = |key: &str, max: u64| -> Result<u64, String> {
            let value = fields
                .get(key)
                .ok_or_else(|| format!("missing profile field: {key}"))?
                .parse::<u64>()
                .map_err(|_| format!("invalid profile value: {key}"))?;
            if value > max {
                return Err(format!("profile value out of range: {key}"));
            }
            Ok(value)
        };
        // Older 1.0 profiles also stored side_light fields; those writers were
        // removed, but reading them keeps existing profiles loadable.
        let st = Self {
            effect: number("effect", (PANEL_EFFECTS - 1) as u64)? as usize,
            brightness: number("brightness", 5)? as u8,
            speed: number("speed", 2)? as u8,
            select: number("select", 2)? as u8,
        };
        st.validate()?;
        Ok(st)
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

fn profiles_dir() -> PathBuf {
    State::path().with_file_name("profiles")
}

fn profile_path(root: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || name.len() > 48
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("profile name must contain 1..48 ASCII letters, digits, - or _".into());
    }
    Ok(root.join(format!("{name}.json")))
}

fn profile_names(root: &std::path::Path) -> Result<Vec<String>, String> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("could not read profiles: {e}")),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
            if profile_path(root, name).is_ok() {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
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

    fn set_feature(&self, payload: [u8; 7]) -> io::Result<()> {
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
        if let Some(error) = error {
            Err(error)
        } else {
            Ok(())
        }
    }

    fn get_feature(&self) -> io::Result<[u8; 8]> {
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

fn send_panel(dev: &Device, st: &State) -> Result<(), String> {
    st.validate()?;
    dev.set_feature([
        0xFF,
        0xFF,
        st.effect as u8,
        st.brightness,
        st.speed,
        st.select,
        0x00,
    ])
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
fn transfer_exact(dev: &Device, send: bool, report: *mut u8) -> io::Result<()> {
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

fn query_firmware(dev: &Device) -> Result<FirmwareResponse, String> {
    let start = std::time::Instant::now();
    let mut request = FIRMWARE_REQUEST;
    transfer_exact(dev, true, request.as_mut_ptr())
        .map_err(|e| format!("could not send firmware query: {} ({})", e, permission_hint(&e)))?;
    thread::sleep(Duration::from_millis(50));
    let mut raw = FIRMWARE_REQUEST;
    transfer_exact(dev, false, raw.as_mut_ptr())
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
        "{{\"panel\":[{}],\"panel_brightness_max\":5,\"speed_max\":2,\"select_max\":2}}",
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
  lecoo-gk301 profiles [--json]            list saved profiles\n\
  lecoo-gk301 profile-save <name>          save lighting settings\n\
  lecoo-gk301 profile-load <name>          apply saved lighting settings\n\
  lecoo-gk301 profile-delete <name>        delete a saved profile\n\
  lecoo-gk301 doctor                       connection and debug information\n\
  lecoo-gk301 response [--json]            validated firmware return path\n\
  lecoo-gk301 restore                      apply last lighting settings\n\
  lecoo-gk301 dev                          show device node\n\
  lecoo-gk301 status [--json]              show last applied state\n\
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
            | "profiles"
            | "profile-save"
            | "profile-delete"
    ) {
        thread::sleep(Duration::from_millis(110));
    }
    match cmd {
        "capabilities" => println!("{}", capabilities_json()),
        "profiles" => {
            let names = profile_names(&profiles_dir())?;
            if has_flag(&args, "--json") {
                println!(
                    "[{}]",
                    names
                        .iter()
                        .map(|n| format!("\"{n}\""))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            } else {
                for name in names {
                    println!("{name}");
                }
            }
        }
        "profile-save" | "profile-load" | "profile-delete" => {
            let name = args
                .get(1)
                .ok_or("usage: profile-save|profile-load|profile-delete <name>")?;
            let path = profile_path(&profiles_dir(), name)?;
            match cmd {
                "profile-save" => {
                    let st = State::load();
                    st.validate()?;
                    st.save_to(&path)
                        .map_err(|e| format!("could not save profile: {e}"))?;
                    println!("Profile saved: {name}");
                }
                "profile-load" => {
                    let st = State::load_from(&path)?;
                    let dev = Device::open_detected(device_arg)?;
                    send_panel(&dev, &st)?;
                    st.save()?;
                    println!("Profile applied: {name}");
                }
                _ => {
                    fs::remove_file(&path).map_err(|e| format!("could not delete profile: {e}"))?;
                    println!("Profile deleted: {name}");
                }
            }
        }
        "doctor" => {
            let dev = Device::open_detected(device_arg)?;
            println!("device: {}", dev.path.display());
            println!("state: {}", State::path().display());
            println!(
                "debug: {}",
                State::path().with_file_name("debug.log").display()
            );
            let resp = query_firmware(&dev)?;
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
            send_panel(&dev, &st)?;
            println!("Last saved lighting settings applied");
        }
        "response" => {
            let dev = Device::open_detected(device_arg)?;
            let resp = query_firmware(&dev)?;
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
            match path {
                Some(p) => {
                    let writable = OpenOptions::new().read(true).write(true).open(&p).is_ok();
                    println!("{}", p.display());
                    println!("writable: {}", writable);
                }
                None => {
                    println!("not found (1a2c:9df4 interface 1)");
                }
            }
        }
        "status" => {
            let st = State::load();
            if has_flag(&args, "--json") {
                println!("{}", st.json());
            } else {
                println!("effect    : {} ({})", st.effect, st.name());
                println!("brightness: {}/5", st.brightness);
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
            send_panel(&dev, &st)?;
            st.save()?;
            println!(
                "panel: {} ({}) brightness {}/5 speed {}/2",
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
            send_panel(&dev, &st)?;
            st.save()?;
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
            send_panel(&dev, &st)?;
            st.save()?;
            println!("brightness: {}/13", st.brightness);
        }
        "preset" => {
            let name = args
                .get(1)
                .ok_or("usage: lecoo-gk301 preset <silent|game|night|party|off|random>")?;
            let st = preset_state(name)?;
            let dev = Device::open_detected(device_arg)?;
            send_panel(&dev, &st)?;
            st.save()?;
            println!(
                "preset: {} · panel {} ({}) {}/5 speed {}/2",
                name,
                st.effect,
                st.name(),
                st.brightness,
                st.speed
            );
        }
        "info" => {
            let dev = Device::open_detected(device_arg)?;
            dev.set_feature([0xF1, 0, 0, 0, 0, 0, 0])
                .map_err(|e| format!("could not send query: {} ({})", e, permission_hint(&e)))?;
            thread::sleep(Duration::from_millis(50));
            let resp = dev
                .get_feature()
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
                .get_feature()
                .map_err(|e| format!("could not read: {} ({})", e, permission_hint(&e)))?;
            println!("{}", hex(&resp));
            println!("(this readback carries firmware data, not lighting state)");
        }
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
        assert_eq!(firmware_string(&[0x07, 0x01, 0x16, 0, 0, 0, 0, 0]).as_deref(), Ok("1.16"));
        assert!(firmware_string(&[0x09, 0x01, 0x16, 0, 0, 0, 0, 0]).is_err());
        assert!(firmware_string(&[0x07, 0x00, 0x00, 0, 0, 0, 0, 0]).is_err());
        assert!(firmware_string(&[0x07, 0x01, 0xFA, 0, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn profile_names_cannot_escape_directory() {
        for name in [
            "",
            "../state",
            "/tmp/p",
            "a/b",
            "a.b",
            "ışık",
            &"a".repeat(49),
        ] {
            assert!(profile_path(std::path::Path::new("/tmp/profiles"), name).is_err());
        }
        assert!(profile_path(std::path::Path::new("/tmp/profiles"), "Game_1-blue").is_ok());
    }

    #[test]
    fn profile_roundtrip_and_corruption_rejection() {
        let root = env::temp_dir().join(format!(
            "lecoo-profiles-{}-{}",
            std::process::id(),
            random_seed()
        ));
        let path = profile_path(&root, "gaming").unwrap();
        let st = State {
            effect: 13,
            brightness: 3,
            speed: 1,
            select: 2,
        };
        st.save_to(&path).unwrap();
        assert_eq!(State::load_from(&path).unwrap().json(), st.json());
        for text in [
            "{}",
            "garbage",
            "{\"effect\":0}",
            &st.json().replace("\"speed\":1", "\"speed\":256"),
            &st.json().replace("\"speed\":1", "\"speed\":1.5"),
            &st.json().replace("\"speed\":1", "\"speed\":-1"),
            &st.json().replace("\"speed\":1", "\"speed\":1,\"speed\":2"),
        ] {
            fs::write(&path, text).unwrap();
            assert!(State::load_from(&path).is_err(), "accepted {text}");
        }
        fs::remove_dir_all(root).unwrap();
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
