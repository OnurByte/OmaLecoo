//! The HID Blackbox: a bounded, local history of the vendor-HID exchanges
//! this tool performed. It never records keyboard input reports or typed keys.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};

use crate::logging::{json_escape, utc_timestamp};
use crate::protocol::hex;
use crate::state::State;

/// Maximum size of the history before it is rotated once.
pub const BLACKBOX_LIMIT: usize = 1_048_576;

/// Records one HID exchange. Recording is best effort: a log failure never
/// turns a successful device command into a failure.
pub fn record_blackbox(
    origin: &str,
    report: &[u8; 8],
    rc: i32,
    error: Option<&io::Error>,
    result: &str,
) {
    let path = State::sibling("blackbox.jsonl");
    let rotated = State::sibling("blackbox.jsonl.1");
    let entry = format!(
        "{{\"utc\":\"{}\",\"origin\":\"{}\",\"report\":\"{}\",\"syscall\":{{\"return\":{},\"error\":{}}},\"result\":\"{}\"}}\n",
        utc_timestamp(),
        json_escape(origin),
        hex(report),
        rc,
        error
            .map(|e| format!("\"{}\"", json_escape(&e.to_string())))
            .unwrap_or_else(|| "null".into()),
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

/// The retained history as a JSON array, newest entry first.
pub fn blackbox_json() -> String {
    let current = State::sibling("blackbox.jsonl");
    let rotated = State::sibling("blackbox.jsonl.1");
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
