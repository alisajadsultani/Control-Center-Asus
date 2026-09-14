//! The daemon's authorization gate. D-Bus (`dbus/org.controlcenter.Daemon1.conf`)
//! only answers "can this process reach the daemon at all"; polkit answers
//! the finer-grained "is *this caller* allowed to do *this action* now."

use zbus::connection::Connection;
use zbus::message::Header;
use zbus_polkit::policykit1::{AuthorityProxy, CheckAuthorizationFlags, Subject};

use crate::error::ControlError;

/// Asks polkit whether the sender of `header` is authorized for
/// `action_id` (registered in `polkit/org.controlcenter.daemon.policy`).
///
/// `Subject::new_for_message_header` builds the subject from the D-Bus
/// sender -- polkit resolves that to a PID/UID itself, so we never trust a
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
