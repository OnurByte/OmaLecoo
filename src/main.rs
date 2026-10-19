//! The `lecoo-gk301` binary.
//!
//! The controller is the only HID writer in the plugin. It runs one command
//! per invocation; the Quickshell service and the panel call it with fixed
//! argument arrays, and every writer shares the same process lock.

use std::env;
use std::process::ExitCode;

use omalecoo::commands;
use omalecoo::lock::command_lock;
use omalecoo::logging::debug_log;
use omalecoo::sys;

fn main() -> ExitCode {
    // Rust installs SIG_IGN for SIGPIPE, which turns `lecoo-gk301 list | head`
    // into a panic (and an abort + core dump with panic=abort). Restore the
    // traditional pipe behaviour: exit quietly when the reader goes away.
    unsafe {
        sys::signal(sys::SIGPIPE, 0);
    }
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            debug_log(&format!("ERROR {}", e));
            eprintln!("lecoo-gk301: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    // The lock is taken before the command runs, so a panel action and an
    // automatic rule can never interleave their HID reports.
    let _lock = command_lock()?;
    commands::dispatch(args)
}
