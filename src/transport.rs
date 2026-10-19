//! HID feature-report transport.
//!
//! The real path is a `HIDIOCSFEATURE` / `HIDIOCGFEATURE` ioctl on the
//! hidraw node. The [`HidTransport`] trait keeps the device layer testable
//! without hardware: offline checks substitute their own transport instead of
//! intercepting libc behind an `LD_PRELOAD` shim.

use std::ffi::c_ulong;
use std::fs::File;
use std::io;
use std::os::unix::io::AsRawFd;

use crate::sys;

/// Length of a GK301 feature report, including the report ID byte.
pub const REPORT_SIZE: usize = 8;

const IOC_WRITE: c_ulong = 1;
const IOC_READ: c_ulong = 2;

/// The `_IOC` encoding of a HID feature request.
fn ioc(dir: c_ulong, ty: u8, nr: u8, size: usize) -> c_ulong {
    (dir << 30) | ((size as c_ulong) << 16) | ((ty as c_ulong) << 8) | nr as c_ulong
}

/// Request code for `HIDIOCSFEATURE` with the given report length.
pub fn hid_set_feature(size: usize) -> c_ulong {
    ioc(IOC_WRITE | IOC_READ, b'H', 0x06, size)
}

/// Request code for `HIDIOCGFEATURE` with the given report length.
pub fn hid_get_feature(size: usize) -> c_ulong {
    ioc(IOC_WRITE | IOC_READ, b'H', 0x07, size)
}

/// A feature-report transport to one HID device.
pub trait HidTransport {
    /// Sends one feature report; returns the number of bytes transferred.
    fn set_feature(&self, report: &mut [u8; REPORT_SIZE]) -> io::Result<usize>;
    /// Reads one feature report; returns the number of bytes transferred.
    fn get_feature(&self, report: &mut [u8; REPORT_SIZE]) -> io::Result<usize>;
}

/// The production transport: a `HIDIOCSFEATURE` / `HIDIOCGFEATURE` ioctl.
pub struct HidrawTransport {
    file: File,
}

impl HidrawTransport {
    /// Wraps an open hidraw file handle.
    pub fn new(file: File) -> Self {
        Self { file }
    }

    fn transfer(&self, request: c_ulong, report: &mut [u8; REPORT_SIZE]) -> io::Result<usize> {
        let rc = unsafe { sys::ioctl(self.file.as_raw_fd(), request, report.as_mut_ptr()) };
        if rc < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(rc as usize)
        }
    }
}

impl HidTransport for HidrawTransport {
    fn set_feature(&self, report: &mut [u8; REPORT_SIZE]) -> io::Result<usize> {
        self.transfer(hid_set_feature(REPORT_SIZE), report)
    }

    fn get_feature(&self, report: &mut [u8; REPORT_SIZE]) -> io::Result<usize> {
        self.transfer(hid_get_feature(REPORT_SIZE), report)
    }
}
