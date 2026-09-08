//! The daemon's authorization gate.
//!
//! `set_charge_limit` calls [`authorize`] before it touches hardware.
//! D-Bus itself only answers "can this process talk to the daemon at all"
//! (see `dbus/org.controlcenter.Daemon1.conf`); polkit is what answers the
//! finer-grained "is *this specific caller* allowed to do *this specific
//! action* right now" -- including prompting for authentication if the
//! system is configured to require it.

use zbus::connection::Connection;
use zbus::message::Header;
use zbus_polkit::policykit1::{AuthorityProxy, CheckAuthorizationFlags, Subject};

use crate::error::ControlError;

/// Asks polkit whether the sender of `header` is authorized for
/// `action_id`, as registered in `polkit/org.controlcenter.daemon.policy`.
///
/// We identify the caller with `Subject::new_for_message_header`, which
/// builds a `system-bus-name` subject from the D-Bus sender -- polkit then
/// resolves that back to a PID/UID on its own, so we never have to trust a
/// UID the client claims to be.
pub async fn authorize(
    connection: &Connection,
    header: &Header<'_>,
    action_id: &str,
) -> Result<(), ControlError> {
    let authority = AuthorityProxy::new(connection)
        .await
        .map_err(ControlError::PolkitUnavailable)?;

    let subject = Subject::new_for_message_header(header).map_err(|err| {
        tracing::warn!(%err, "could not build a polkit subject for the caller");
        ControlError::NotAuthorized
    })?;

    // AllowUserInteraction lets polkit pop an authentication prompt (e.g. a
    // password dialog) if the configured policy requires one. Without this
    // flag, a "requires auth" action is reported as not authorized outright
    // rather than offering the caller a chance to authenticate.
    let result = authority
        .check_authorization(
            &subject,
            action_id,
            &std::collections::HashMap::new(),
            CheckAuthorizationFlags::AllowUserInteraction.into(),
            "",
        )
        .await
        .map_err(ControlError::PolkitUnavailable)?;

    if result.is_authorized {
        Ok(())
    } else {
        Err(ControlError::NotAuthorized)
    }
}
