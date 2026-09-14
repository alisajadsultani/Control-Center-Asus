//! Sysfs I/O for the platform-profile control. No D-Bus here -- mirrors
//! `battery.rs`: read/validate/write/verify against one file.
//!
//! Not wired into `main.rs` yet, hence `dead_code` allowed here.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use crate::error::ControlError;

const ACPI_ROOT: &str = "/sys/firmware/acpi";
const PROFILE_FILE: &str = "platform_profile";
const CHOICES_FILE: &str = "platform_profile_choices";

/// Unlike the battery threshold file (which lives under a per-device
/// directory that varies by hardware), `platform_profile` is always at
/// this one fixed path -- so this is just an existence check, not a scan.
pub fn discover_platform_path() -> Option<PathBuf> {
    let path = Path::new(ACPI_ROOT).join(PROFILE_FILE);
    path.is_file().then_some(path)
}

pub fn read_profile(path: &Path) -> Result<String, ControlError> {
    let raw = std::fs::read_to_string(path).map_err(ControlError::SysfsRead)?;
    Ok(raw.trim().to_string())
}

/// Checks `profile` against the live `platform_profile_choices` list.
pub fn validate(profile: &str) -> Result<(), ControlError> {
    let choices_path = Path::new(ACPI_ROOT).join(CHOICES_FILE);
    let contents = std::fs::read_to_string(&choices_path).map_err(ControlError::SysfsRead)?;
    let choices: Vec<&str> = contents.split_whitespace().collect();

    if choices.contains(&profile) {
        Ok(())
    } else {
        Err(ControlError::InvalidProfile {
            value: profile.to_string(),
            choices: choices.into_iter().map(str::to_string).collect(),
        })
    }
}

/// Returns the *applied* profile, read back after writing -- same
/// validate -> write -> read-back-to-verify convention as `write_limit`.
pub fn write_profile(path: &Path, profile: &str) -> Result<String, ControlError> {
    validate(profile)?;
    std::fs::write(path, profile).map_err(ControlError::SysfsWrite)?;
    read_profile(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_a_real_file() {
        let dir = std::env::temp_dir()
            .join(format!("controlcenterd-test-platform-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(PROFILE_FILE);
        std::fs::write(&path, "balanced\n").unwrap();

        assert_eq!(read_profile(&path).unwrap(), "balanced");

        std::fs::remove_dir_all(&dir).ok();
    }
}
