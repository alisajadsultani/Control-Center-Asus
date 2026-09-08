//! Talks to the one piece of hardware this feature touches: the
//! `charge_control_end_threshold` sysfs attribute that the kernel's
//! `power_supply` class exposes for batteries whose driver supports it
//! (on ASUS ROG laptops that's wired up by `asus_wmi`).
//!
//! This module knows nothing about D-Bus -- it's deliberately just
//! "read/validate/write/verify" against one file, independent of whatever
//! sits on top of it.

use std::path::{Path, PathBuf};

use crate::error::ControlError;

/// The asus-wmi driver on this hardware accepts any integer in this range.
/// Some other ASUS models only honor specific steps (commonly 60/80/100)
/// and clamp silently rather than reject -- `write_limit` below is what
/// catches that, by reading the value back after writing it.
pub const MIN_CHARGE_LIMIT: u8 = 20;
pub const MAX_CHARGE_LIMIT: u8 = 100;

const POWER_SUPPLY_ROOT: &str = "/sys/class/power_supply";
const THRESHOLD_FILE: &str = "charge_control_end_threshold";

/// Looks for the first power-supply device that exposes a
/// `charge_control_end_threshold` file. Returns `None` if nothing on this
/// system supports the feature -- this is feature *detection*, not an
/// assumption baked in at compile time, since the attribute's presence
/// depends on the kernel driver and BIOS/EC firmware.
pub fn discover_threshold_path() -> Option<PathBuf> {
    let entries = std::fs::read_dir(POWER_SUPPLY_ROOT).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path().join(THRESHOLD_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Reads the currently applied charge limit straight from hardware (no
/// caching) so callers always see ground truth.
pub fn read_limit(path: &Path) -> Result<u8, ControlError> {
    let raw = std::fs::read_to_string(path).map_err(ControlError::SysfsRead)?;
    raw.trim()
        .parse::<u8>()
        .map_err(|_| ControlError::UnexpectedSysfsValue(raw.trim().to_string()))
}

/// Bounds-checks a requested limit before it ever reaches a `write()`
/// syscall.
pub fn validate(limit: u8) -> Result<(), ControlError> {
    if (MIN_CHARGE_LIMIT..=MAX_CHARGE_LIMIT).contains(&limit) {
        Ok(())
    } else {
        Err(ControlError::OutOfRange {
            value: limit,
            min: MIN_CHARGE_LIMIT,
            max: MAX_CHARGE_LIMIT,
        })
    }
}

/// Validates, writes, then reads back to confirm what hardware actually
/// applied. Returns the *applied* value, which callers should treat as
/// authoritative -- it may differ from `limit` on hardware that only
/// supports discrete steps.
pub fn write_limit(path: &Path, limit: u8) -> Result<u8, ControlError> {
    validate(limit)?;
    std::fs::write(path, limit.to_string()).map_err(ControlError::SysfsWrite)?;
    read_limit(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_out_of_range() {
        assert!(validate(10).is_err());
        assert!(validate(101).is_err());
        assert!(validate(20).is_ok());
        assert!(validate(100).is_ok());
    }

    #[test]
    fn round_trips_through_a_real_file() {
        let dir = std::env::temp_dir().join(format!("controlcenterd-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(THRESHOLD_FILE);
        std::fs::write(&path, "100\n").unwrap();

        assert_eq!(read_limit(&path).unwrap(), 100);
        assert_eq!(write_limit(&path, 80).unwrap(), 80);
        assert_eq!(read_limit(&path).unwrap(), 80);

        std::fs::remove_dir_all(&dir).ok();
    }
}
