//! The single udev rule this plugin owns, and its install/remove path.
//!
//! The rule is the only system file OmaLecoo writes. The controller is the
//! only writer of that content, so the file it compares against is the
//! single source of truth; it refuses to overwrite or delete a rule whose
//! contents differ from the one it owns.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::sys;

/// Absolute path of the owned rule.
pub const UDEV_RULE_PATH: &str = "/etc/udev/rules.d/70-lecoo-gk301.rules";

/// The exact content of the owned rule: `uaccess` for the GK301 vendor
/// interface, so the desktop session may open the hidraw node.
pub const UDEV_RULE_CONTENT: &str = "# Lecoo GK301 (1a2c:9df4) vendor HID interface (report ID 7)\nSUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"1a2c\", ATTRS{idProduct}==\"9df4\", ENV{ID_USB_INTERFACE_NUM}==\"01\", TAG+=\"uaccess\"\n";

/// Installs or removes the owned rule. Root only.
pub fn udev_command(action: &str) -> Result<(), String> {
    if unsafe { sys::geteuid() } != 0 {
        return Err("udev install/remove must be run as root (for example through pkexec)".into());
    }
    let path = Path::new(UDEV_RULE_PATH);
    match action {
        "install" => match fs::read_to_string(path) {
            Ok(existing) if existing == UDEV_RULE_CONTENT => {}
            Ok(_) => {
                return Err(format!(
                    "refusing to overwrite {} because its contents differ",
                    UDEV_RULE_PATH
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
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
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
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
