//! White-box transport checks: one main-panel packet per operation, the
//! validated firmware response and its failure matrix.
//!
//! The HID transport is substituted in process, so no C interception shim and
//! no `LD_PRELOAD` are involved. No USB hardware is touched.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use omalecoo::commands::preset_state;
use omalecoo::device::{firmware_report_json, query_firmware, send_panel, Device};
use omalecoo::protocol::{hex, FIRMWARE_REQUEST};
use omalecoo::state::State;
use omalecoo::transport::{HidTransport, REPORT_SIZE};

/// A transport that records every written report and answers with a fixture.
struct FakeTransport {
    written: Mutex<Vec<[u8; REPORT_SIZE]>>,
    response: [u8; REPORT_SIZE],
    short_set: bool,
    short_get: bool,
}

impl FakeTransport {
    fn new(response: [u8; REPORT_SIZE]) -> Self {
        Self {
            written: Mutex::new(Vec::new()),
            response,
            short_set: false,
            short_get: false,
        }
    }

    fn packets(&self) -> Vec<[u8; REPORT_SIZE]> {
        self.written.lock().expect("transport lock").clone()
    }
}

/// A shareable handle to a fixture, so the test can inspect the recorded
/// reports after the device has been handed the transport.
struct SharedTransport(Arc<FakeTransport>);

impl HidTransport for SharedTransport {
    fn set_feature(&self, report: &mut [u8; REPORT_SIZE]) -> io::Result<usize> {
        self.0.written.lock().expect("transport lock").push(*report);
        if self.0.short_set {
            Ok(3)
        } else {
            Ok(REPORT_SIZE)
        }
    }

    fn get_feature(&self, report: &mut [u8; REPORT_SIZE]) -> io::Result<usize> {
        *report = self.0.response;
        if self.0.short_get {
            Ok(3)
        } else {
            Ok(REPORT_SIZE)
        }
    }
}

fn firmware_fixture() -> [u8; REPORT_SIZE] {
    [7, 1, 0x16, 0, 0, 0, 0, 0]
}

fn device_with(transport: &Arc<FakeTransport>) -> Device {
    Device::with_transport(
        PathBuf::from("/dev/hidraw0"),
        Box::new(SharedTransport(Arc::clone(transport))),
    )
}

#[test]
fn transport_checks() {
    // Keep the recorder out of the real state directory.
    let tmp = std::env::temp_dir().join(format!(
        "omalecoo-transport-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&tmp).expect("temp state dir");
    std::env::set_var("XDG_STATE_HOME", &tmp);

    // A single main-panel packet per operation, panel path only.
    for state in [
        preset_state("night").expect("night preset"),
        State::default(),
    ] {
        let transport = Arc::new(FakeTransport::new(firmware_fixture()));
        let device = device_with(&transport);
        send_panel(&device, &state, "panel").expect("panel command");
        let packets = transport.packets();
        assert_eq!(packets.len(), 1, "{:?}", packets);
        assert!(hex(&packets[0]).starts_with("07 FF FF "), "{:?}", packets);
        assert_ne!(packets[0][3], 20, "effect byte must stay in range");
    }

    // The validated firmware response and its report.
    let transport = Arc::new(FakeTransport::new(firmware_fixture()));
    let device = device_with(&transport);
    let response = query_firmware(&device, "panel").expect("firmware query");
    assert_eq!(response.firmware, "1.16");
    let report = firmware_report_json(&response);
    for expected in [
        "\"device\":\"/dev/hidraw0\"",
        "\"request\":\"07 F1 00 00 00 00 00 00\"",
        "\"raw\":\"07 01 16 00 00 00 00 00\"",
        "\"firmware\":\"1.16\"",
        "\"kind\":\"firmware\"",
        "\"lighting_readback\":false",
        "\"roundtrip_ms\":",
    ] {
        assert!(report.contains(expected), "missing {expected}: {report}");
    }
    assert_eq!(FIRMWARE_REQUEST[0], 7);

    // Every tampered fixture must fail.
    for (label, fixture) in [
        ("bad-id", [9, 1, 0x16, 0, 0, 0, 0, 0]),
        ("empty", [7, 0, 0, 0, 0, 0, 0, 0]),
        ("bad-version", [7, 1, 0xfa, 0, 0, 0, 0, 0]),
    ] {
        let transport = Arc::new(FakeTransport::new(fixture));
        let device = device_with(&transport);
        assert!(
            query_firmware(&device, "panel").is_err(),
            "fixture {label} must fail"
        );
    }

    // A short transfer on either side of the query must fail.
    for short_set in [true, false] {
        let mut fixture = FakeTransport::new(firmware_fixture());
        fixture.short_set = short_set;
        fixture.short_get = !short_set;
        let transport = Arc::new(fixture);
        let device = device_with(&transport);
        assert!(
            query_firmware(&device, "panel").is_err(),
            "short transfer (set={short_set}) must fail"
        );
    }

    let _ = fs::remove_dir_all(&tmp);
}
