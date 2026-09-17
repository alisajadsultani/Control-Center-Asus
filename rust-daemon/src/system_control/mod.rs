//! System Control: battery, platform profile, and other laptop-setting
//! controls (per `config/project-description.txt`).

mod battery;
mod battery_iface;
mod platform_profile;
mod platform_profile_iface;
mod updates;
mod updates_iface;

pub use battery::discover_threshold_path;
pub use battery_iface::BatteryInterface;
#[allow(unused_imports)]
pub use platform_profile::discover_platform_path;
#[allow(unused_imports)]
pub use platform_profile_iface::PlatformProfileInterface;
pub use updates_iface::UpdatesInterface;
