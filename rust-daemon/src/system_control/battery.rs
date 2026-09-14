//! Sysfs I/O for the battery charge-limit control. No D-Bus here --
//! deliberately just read/validate/write/verify against one file.

use std::path::{Path, PathBuf};

use crate::error::ControlError;

/// Some ASUS models only honor specific steps (e.g. 60/80/100) and clamp
/// silently rather than reject -- `write_limit` below catches that by
/// reading the value back after writing.
pub const MIN_CHARGE_LIMIT: u8 = 20;
pub const MAX_CHARGE_LIMIT: u8 = 100;

const POWER_SUPPLY_ROOT: &str = "/sys/class/power_supply";
const THRESHOLD_FILE: &str = "charge_control_end_threshold";

/// `None` if nothing on this system exposes the attribute -- presence
/// depends on the kernel driver and BIOS/EC firmware, so this is runtime
/// detection, not a compile-time assumption.
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

pub fn read_limit(path: &Path) -> Result<u8, ControlError> {
    let raw = std::fs::read_to_string(path).map_err(ControlError::SysfsRead)?;
    raw.trim()
        .parse::<u8>()
        .map_err(|_| ControlError::UnexpectedSysfsValue(raw.trim().to_string()))
}

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

/// Returns the *applied* value, which may differ from `limit` on hardware
/// that only supports discrete steps.
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
        let dir =
            std::env::temp_dir().join(format!("controlcenterd-test-battery-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(THRESHOLD_FILE);
        std::fs::write(&path, "100\n").unwrap();

        assert_eq!(read_limit(&path).unwrap(), 100);
        assert_eq!(write_limit(&path, 80).unwrap(), 80);
        assert_eq!(read_limit(&path).unwrap(), 80);

        std::fs::remove_dir_all(&dir).ok();
    }
}
