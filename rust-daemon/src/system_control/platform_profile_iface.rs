//! The `org.controlcenter.Daemon1.PlatformProfile` D-Bus interface.
//!
//! Not wired into `main.rs` yet, hence `dead_code` allowed here.
#![allow(dead_code)]

use std::path::PathBuf;

use zbus::connection::Connection;
use zbus::interface;
use zbus::message::Header;
use zbus::object_server::SignalEmitter;

use super::platform_profile;
use crate::error::ControlError;
use crate::polkit;

/// Must match the `<action id=...>` in `polkit/org.controlcenter.daemon.policy`.
const ACTION_SET_PLATFORM_PROFILE: &str = "org.controlcenter.daemon.set-platform_profile";

pub struct PlatformProfileInterface {
    /// `None` means this system doesn't expose the sysfs attribute; every
    /// method below treats that as `ControlError::Unsupported`.
    profile_path: Option<PathBuf>,
}

impl PlatformProfileInterface {
    pub fn new(profile_path: Option<PathBuf>) -> Self {
        Self { profile_path }
    }
}

#[interface(name = "org.controlcenter.Daemon1.PlatformProfile")]
impl PlatformProfileInterface {
    #[zbus(property)]
    async fn supported(&self) -> bool {
        self.profile_path.is_some()
    }

    #[zbus(property)]
    async fn profile(&self) -> zbus::fdo::Result<String> {
        let path = self
            .profile_path
            .as_ref()
            .ok_or(ControlError::Unsupported)?;
        Ok(platform_profile::read_profile(path)?)
    }

    /// Returns the value hardware actually applied.
    async fn set_profile(
        &self,
        profile: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<String> {
        polkit::authorize(connection, &header, ACTION_SET_PLATFORM_PROFILE).await?;

        let path = self
            .profile_path
            .as_ref()
            .ok_or(ControlError::Unsupported)?;
        let applied = platform_profile::write_profile(path, &profile)?;

        // `profile` being `#[zbus(property)]` generates this method for
        // free -- it emits the standard PropertiesChanged signal.
        if let Err(err) = self.profile_changed(&emitter).await {
            tracing::warn!(%err, "failed to emit Profile PropertiesChanged notification");
        }

        Ok(applied)
    }
}
