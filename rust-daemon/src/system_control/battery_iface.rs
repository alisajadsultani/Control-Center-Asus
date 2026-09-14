//! The `org.controlcenter.Daemon1.Battery` D-Bus interface.

use std::path::PathBuf;

use zbus::connection::Connection;
use zbus::interface;
use zbus::message::Header;
use zbus::object_server::SignalEmitter;

use super::battery;
use crate::error::ControlError;
use crate::polkit;

/// Must match the `<action id=...>` in `polkit/org.controlcenter.daemon.policy`.
const ACTION_SET_CHARGE_LIMIT: &str = "org.controlcenter.daemon.set-charge-limit";

pub struct BatteryInterface {
    /// `None` means this system doesn't expose the sysfs attribute; every
    /// method below treats that as `ControlError::Unsupported`.
    threshold_path: Option<PathBuf>,
}

impl BatteryInterface {
    pub fn new(threshold_path: Option<PathBuf>) -> Self {
        Self { threshold_path }
    }
}

#[interface(name = "org.controlcenter.Daemon1.Battery")]
impl BatteryInterface {
    #[zbus(property)]
    async fn supported(&self) -> bool {
        self.threshold_path.is_some()
    }

    #[zbus(property)]
    async fn charge_limit(&self) -> zbus::fdo::Result<u8> {
        let path = self
            .threshold_path
            .as_ref()
            .ok_or(ControlError::Unsupported)?;
        Ok(battery::read_limit(path)?)
    }

    /// Returns the value hardware actually applied, which callers should
    /// treat as authoritative -- it can differ from `limit` on hardware
    /// that only supports discrete steps.
    async fn set_charge_limit(
        &self,
        limit: u8,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<u8> {
        polkit::authorize(connection, &header, ACTION_SET_CHARGE_LIMIT).await?;

        let path = self
            .threshold_path
            .as_ref()
            .ok_or(ControlError::Unsupported)?;
        let applied = battery::write_limit(path, limit)?;

        // `charge_limit` being `#[zbus(property)]` generates this method
        // for free -- it emits the standard PropertiesChanged signal.
        if let Err(err) = self.charge_limit_changed(&emitter).await {
            tracing::warn!(%err, "failed to emit ChargeLimit PropertiesChanged notification");
        }

        if applied != limit {
            tracing::warn!(
                requested = limit,
                applied,
                "hardware applied a different charge limit than requested"
            );
        }

        Ok(applied)
    }
}
