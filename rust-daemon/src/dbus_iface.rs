//! The `org.controlcenter.Daemon1.Battery` D-Bus interface.
//!
//! `set_charge_limit` is gated by polkit (see `crate::polkit`) before it
//! touches hardware. Rate limiting and persistence are still separate
//! boxes in the system design, not added yet.

use std::path::PathBuf;

use zbus::connection::Connection;
use zbus::interface;
use zbus::message::Header;
use zbus::object_server::SignalEmitter;

use crate::battery;
use crate::error::ControlError;
use crate::polkit;

/// polkit action ID this method requires -- must match the `<action id=...>`
/// in `polkit/org.controlcenter.daemon.policy`.
const ACTION_SET_CHARGE_LIMIT: &str = "org.controlcenter.daemon.set-charge-limit";

pub struct BatteryInterface {
    /// `None` means this system doesn't expose the sysfs attribute at all
    /// (detected once at startup in `main.rs`); every method below treats
    /// that as `ControlError::Unsupported` rather than panicking.
    threshold_path: Option<PathBuf>,
}

impl BatteryInterface {
    pub fn new(threshold_path: Option<PathBuf>) -> Self {
        Self { threshold_path }
    }
}

#[interface(name = "org.controlcenter.Daemon1.Battery")]
impl BatteryInterface {
    /// Whether this system exposes a battery charge-limit control at all.
    /// A GUI/CLI should check this before offering the control in the UI.
    #[zbus(property)]
    async fn supported(&self) -> bool {
        self.threshold_path.is_some()
    }

    /// Current charge limit, read live from hardware (not cached) so it's
    /// always ground truth even if something else on the system changed it.
    #[zbus(property)]
    async fn charge_limit(&self) -> zbus::fdo::Result<u8> {
        let path = self
            .threshold_path
            .as_ref()
            .ok_or(ControlError::Unsupported)?;
        Ok(battery::read_limit(path)?)
    }

    /// Sets the battery charge limit. Requires polkit authorization for
    /// `org.controlcenter.daemon.set-charge-limit`. Returns the value
    /// hardware actually applied, which callers should treat as
    /// authoritative -- it can differ from `limit` on hardware that only
    /// supports discrete steps.
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

        // Not a signal we wrote by hand -- declaring `charge_limit` as a
        // `#[zbus(property)]` above generates this method for free. It
        // emits the standard `org.freedesktop.DBus.Properties.PropertiesChanged`
        // signal, so any generic D-Bus property watcher picks up the change,
        // not just a client written specifically against this interface.
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
