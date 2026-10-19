//! The cross-process command lock.
//!
//! Every writer — the CLI, the bar widget and the Signal Engine — takes this
//! lock before touching the keyboard, so a panel action and an automatic
//! rule can never interleave their HID reports.

use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

use crate::state::State;
use crate::sys;

/// Name of the lock file inside the state directory.
pub const LOCK_FILE: &str = "command.lock";

/// Path of the command lock.
pub fn path() -> PathBuf {
    State::sibling(LOCK_FILE)
}

/// Takes the exclusive command lock. The returned handle keeps it held.
pub fn command_lock() -> Result<fs::File, String> {
    let path = path();
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    // Lock across processes: firmware queries, CLI writes and all panel instances.
    if unsafe { sys::flock(file.as_raw_fd(), sys::LOCK_EX) } < 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    Ok(file)
}
