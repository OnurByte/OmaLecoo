//! Hidden research diagnostics.
//!
//! Both commands are experiments against the live keyboard; neither is part
//! of the product surface. They exist so the recovered protocol behavior can
//! be reproduced without any non-Rust tooling.

use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::device::{query_firmware, send_panel, Device};
use crate::protocol::{hex, REPORT_ID, STARTUP_RECORD};
use crate::state::State;

const O_NONBLOCK: i32 = 0o4000;

/// Replays the recovered Windows startup sequence.
///
/// Without `--send` the command only prints the recovered record. With
/// `--send` it performs the startup-form query and write against the live
/// keyboard: the firmware query, then the stored-record write, each
/// separated by the original delays. Cached lighting state is not touched.
pub fn probe_startup(
    args: &[String],
    device_arg: Option<&str>,
    origin: &str,
) -> Result<(), String> {
    let send = args.iter().any(|a| a == "--send");
    println!("Recovered startup packet: {}", hex(&STARTUP_RECORD));
    if !send {
        println!(
            "No device write. Use --send to reproduce the physically observed startup experiment."
        );
        return Ok(());
    }

    let device = Device::open_detected(device_arg)?;
    if !device.path().starts_with("/dev/hidraw") {
        return Err("GK301 HID node was not found".into());
    }

    // The firmware query of the recovered startup path.
    device
        .set_feature([0xF1, 0, 0, 0, 0, 0, 0], origin)
        .map_err(|e| format!("could not send firmware query: {}", e))?;
    thread::sleep(Duration::from_millis(150));
    let response = device
        .get_feature(origin)
        .map_err(|e| format!("could not read firmware response: {}", e))?;
    println!("Firmware report: {}", hex(&response));

    thread::sleep(Duration::from_secs(2));
    let transferred = device
        .set_feature_counted(
            [
                STARTUP_RECORD[1],
                STARTUP_RECORD[2],
                STARTUP_RECORD[3],
                STARTUP_RECORD[4],
                STARTUP_RECORD[5],
                STARTUP_RECORD[6],
                STARTUP_RECORD[7],
            ],
            origin,
        )
        .map_err(|e| format!("could not send startup record: {}", e))?;
    println!("Transfer result: {}", transferred);

    thread::sleep(Duration::from_secs(2));
    println!(
        "Experiment sent; no state readback is claimed and cached lighting state is unchanged."
    );
    Ok(())
}

/// Hidden research diagnostic (not listed in usage): replays the Windows
/// tool's startup-form sweep — the stored-record write with an unclamped
/// brightness byte — on the live keyboard, and records any vendor input
/// reports from a concurrent non-blocking reader. Results are written to
/// a JSON file; see docs/captures/extended-brightness-experiment.md.
pub fn probe_brightness(
    args: &[String],
    device_arg: Option<&str>,
    origin: &str,
) -> Result<(), String> {
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
        .open(device.path())
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
        &format!(",\"device\":\"{}\"", device.path().display()),
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
                    crate::device::permission_hint(&e)
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
                        crate::device::permission_hint(&e)
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
        device.path().display(),
        stored.join(",\n    ")
    );
    fs::write(&out_path, report)
        .map_err(|e| format!("could not write {}: {e}", out_path.display()))?;
    println!("saved {} events -> {}", stored.len(), out_path.display());
    Ok(())
}
