//! Minimal libc bindings used by the controller.
//!
//! The crate deliberately has no runtime dependency beyond the C library, so
//! the handful of calls it needs are declared here in one place instead of
//! being spread across feature modules.

use std::ffi::{c_int, c_ulong};

extern "C" {
    /// `ioctl(2)` — used for the HID feature-report transfers.
    pub fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    /// `flock(2)` — the cross-process command lock.
    pub fn flock(fd: c_int, operation: c_int) -> c_int;
    /// `signal(2)` — restores the default `SIGPIPE` disposition.
    pub fn signal(signum: c_int, handler: usize) -> usize;
    /// `access(2)` — writability probe for a device node.
    pub fn access(path: *const i8, mode: c_int) -> c_int;
    /// `geteuid(2)` — the privilege check for udev operations.
    pub fn geteuid() -> u32;
}

/// Exclusive `flock` operation.
pub const LOCK_EX: c_int = 2;
/// Write permission bit for `access(2)`.
pub const W_OK: c_int = 2;
/// `SIGPIPE` signal number.
pub const SIGPIPE: c_int = 13;
