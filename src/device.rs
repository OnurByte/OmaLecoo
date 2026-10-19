//! Device discovery and the HID write paths.
//!
//! The controller only ever talks to the GK301's vendor HID interface
//! (interface 1, report ID 7). Discovery reads sysfs; it never opens a
//! report path and never sends a command.

use std::env;
use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::blackbox::record_blackbox;
use crate::logging::debug_log;
use crate::protocol::{hex, panel_payload, FIRMWARE_REQUEST, PID, REPORT_ID, VID};
use crate::state::State;
use crate::sys;
use crate::transport::{HidTransport, HidrawTransport, REPORT_SIZE};

/// An open keyboard device.
pub struct Device {
    transport: Box<dyn HidTransport>,
    path: PathBuf,
}

impl Device {
    /// Opens a hidraw node for writing.
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("{}: {} ({})", path.display(), e, permission_hint(&e)))?;
        debug_log(&format!("device_open path={}", path.display()));
        Ok(Self {
            transport: Box::new(HidrawTransport::new(file)),
            path,
        })
    }

    /// Builds a device around a substituted transport, for offline checks.
    pub fn with_transport(path: PathBuf, transport: Box<dyn HidTransport>) -> Self {
        Self { transport, path }
    }

    /// Opens the detected keyboard, or the explicitly requested node.
    pub fn open_detected(explicit: Option<&str>) -> Result<Self, String> {
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

    /// The hidraw node this device talks to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Sends one main-panel command.
    ///
    /// The transfer must move exactly one report. A short transfer is a
    /// failure, not a partial write: the same rule the firmware query uses.
    /// A successful transfer still only means the device accepted the report,
    /// never that the LEDs changed.
    pub fn set_feature(&self, payload: [u8; 7], origin: &str) -> io::Result<()> {
        self.set_feature_counted(payload, origin).map(|_| ())
    }

    /// Sends one feature report and reports how many bytes were transferred.
    pub fn set_feature_counted(&self, payload: [u8; 7], origin: &str) -> io::Result<usize> {
        let mut buf = [0u8; REPORT_SIZE];
        buf[0] = REPORT_ID;
        buf[1..].copy_from_slice(&payload);
        let result = self.transport.set_feature(&mut buf);
        let (rc, error) = match result {
            Ok(n) if n == REPORT_SIZE => (n as i32, None),
            Ok(n) => (
                n as i32,
                Some(io::Error::other(format!(
                    "short transfer ({n} bytes, expected {REPORT_SIZE})"
                ))),
            ),
            Err(error) => (-1, Some(error)),
        };
        debug_log(&format!(
            "hid ioctl rc={} report={} error={}",
            rc,
            hex(&buf),
            error.as_ref().map(ToString::to_string).unwrap_or_default()
        ));
        record_blackbox(origin, &buf, rc, error.as_ref(), "feature report");
        match error {
            Some(error) => Err(error),
            None => Ok(rc.max(0) as usize),
        }
    }

    /// Reads one feature report.
    ///
    /// A short read is a failure for the same reason the write path treats a
    /// short write as one.
    pub fn get_feature(&self, origin: &str) -> io::Result<[u8; REPORT_SIZE]> {
        let mut buf = [0u8; REPORT_SIZE];
        buf[0] = REPORT_ID;
        let result = self.transport.get_feature(&mut buf);
        let (rc, error) = match result {
            Ok(n) if n == REPORT_SIZE => (n as i32, None),
            Ok(n) => (
                n as i32,
                Some(io::Error::other(format!(
                    "short transfer ({n} bytes, expected {REPORT_SIZE})"
                ))),
            ),
            Err(error) => (-1, Some(error)),
        };
        debug_log(&format!(
            "hid ioctl rc={} report={} error={}",
            rc,
            hex(&buf),
            error.as_ref().map(ToString::to_string).unwrap_or_default()
        ));
        record_blackbox(origin, &buf, rc, error.as_ref(), "firmware acknowledgement");
        match error {
            Some(error) => Err(error),
            None => Ok(buf),
        }
    }
}

/// The recovered firmware answer: a reply, not a lighting readback.
pub struct FirmwareResponse {
    /// The hidraw node that answered.
    pub device: PathBuf,
    /// The raw eight-byte reply.
    pub raw: [u8; REPORT_SIZE],
    /// The parsed firmware version.
    pub firmware: String,
    /// Round-trip duration of the query.
    pub roundtrip_ms: u128,
}

/// Sends the recovered main-panel command for the given settings.
pub fn send_panel(dev: &Device, st: &State, origin: &str) -> Result<(), String> {
    st.validate()?;
    dev.set_feature(panel_payload(st), origin).map_err(|e| {
        format!(
            "could not send panel command: {} ({})",
            e,
            permission_hint(&e)
        )
    })
}

/// Queries the firmware and validates the reply.
pub fn query_firmware(dev: &Device, origin: &str) -> Result<FirmwareResponse, String> {
    let start = std::time::Instant::now();
    let mut request = FIRMWARE_REQUEST;
    transfer_exact(dev, true, &mut request, origin).map_err(|e| {
        format!(
            "could not send firmware query: {} ({})",
            e,
            permission_hint(&e)
        )
    })?;
    thread::sleep(Duration::from_millis(50));
    let mut raw = FIRMWARE_REQUEST;
    transfer_exact(dev, false, &mut raw, origin)
        .map_err(|e| format!("could not read response: {} ({})", e, permission_hint(&e)))?;
    let firmware = firmware_string(&raw)?;
    Ok(FirmwareResponse {
        device: dev.path().to_path_buf(),
        raw,
        firmware,
        roundtrip_ms: start.elapsed().as_millis(),
    })
}

/// The JSON report of a validated firmware response.
pub fn firmware_report_json(resp: &FirmwareResponse) -> String {
    format!(
        "{{\"device\":\"{}\",\"request\":\"{}\",\"raw\":\"{}\",\"firmware\":\"{}\",\"kind\":\"firmware\",\"lighting_readback\":false,\"roundtrip_ms\":{}}}",
        resp.device.display(),
        hex(&FIRMWARE_REQUEST),
        hex(&resp.raw),
        resp.firmware,
        resp.roundtrip_ms
    )
}

/// Feature transfer whose byte count is validated: HIDIOCSFEATURE/HIDIOCGFEATURE
/// must return exactly the report length, never a short result.
fn transfer_exact(
    dev: &Device,
    send: bool,
    report: &mut [u8; REPORT_SIZE],
    origin: &str,
) -> io::Result<()> {
    let result = if send {
        dev.transport.set_feature(report)
    } else {
        dev.transport.get_feature(report)
    };
    let (rc, error) = match result {
        Ok(n) if n == REPORT_SIZE => (n as i32, None),
        Ok(n) => (
            n as i32,
            Some(io::Error::other(format!(
                "short transfer ({n} bytes, expected {REPORT_SIZE})"
            ))),
        ),
        Err(error) => (-1, Some(error)),
    };
    debug_log(&format!(
        "hid {} rc={} report={} error={}",
        if send { "set" } else { "get" },
        rc,
        hex(report),
        error.as_ref().map(ToString::to_string).unwrap_or_default()
    ));
    record_blackbox(
        origin,
        report,
        rc,
        error.as_ref(),
        if send {
            "firmware query"
        } else {
            "firmware acknowledgement"
        },
    );
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Parses the firmware version out of a validated reply.
pub fn firmware_string(raw: &[u8; REPORT_SIZE]) -> Result<String, String> {
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

/// A short explanation when the hidraw node cannot be opened.
pub fn permission_hint(e: &io::Error) -> String {
    if e.raw_os_error() == Some(13) {
        "permission denied: udev rule is not installed".to_string()
    } else {
        e.to_string()
    }
}

/// Finds the GK301 vendor HID node.
pub fn detect_device() -> Option<PathBuf> {
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

/// The device presence report, as JSON. Discovery never opens a report path.
pub fn device_presence_json() -> String {
    match detect_device() {
        Some(path) => {
            format!(
                "{{\"connected\":true,\"path\":\"{}\",\"writable\":{}}}",
                crate::logging::json_escape(&path.display().to_string()),
                path_is_writable(&path)
            )
        }
        None => "{\"connected\":false,\"path\":null,\"writable\":false}".into(),
    }
}

/// Whether a device node can be opened for writing.
pub fn path_is_writable(path: &Path) -> bool {
    CString::new(path.as_os_str().as_encoded_bytes())
        .map(|path| unsafe { sys::access(path.as_ptr(), sys::W_OK) == 0 })
        .unwrap_or(false)
}
