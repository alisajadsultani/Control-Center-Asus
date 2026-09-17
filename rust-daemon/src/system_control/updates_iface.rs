//! The `org.controlcenter.Daemon1.Updates` D-Bus interface.

use zbus::connection::Connection;
use zbus::interface;
use zbus::message::Header;

use super::updates;
use crate::polkit;

/// Must match the `<action id=...>` in `polkit/org.controlcenter.daemon.policy`.
const ACTION_APPLY_OS_UPDATE: &str = "org.controlcenter.daemon.apply-os-update";

#[derive(Default)]
pub struct UpdatesInterface;

#[interface(name = "org.controlcenter.Daemon1.Updates")]
impl UpdatesInterface {
    async fn preview_os(&self) -> zbus::fdo::Result<String> {
        Ok(updates::preview_os().await?)
    }

    async fn apply_os(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<()> {
        polkit::authorize(connection, &header, ACTION_APPLY_OS_UPDATE).await?;
        Ok(updates::apply_os().await?)
    }
}
