use thiserror::Error;

/// Everything that can go wrong in this slice of the daemon, in domain
/// terms rather than raw I/O terms. The `From<ControlError> for
/// zbus::fdo::Error` impl below is the one place that decides what a D-Bus
/// caller actually sees -- callers never get a raw `io::Error` or a path.
#[derive(Debug, Error)]
pub enum ControlError {
    #[error("no battery on this system exposes a charge-limit control")]
    Unsupported,

    #[error("charge limit {value} is out of the supported range {min}-{max}")]
    OutOfRange { value: u8, min: u8, max: u8 },

    #[error("failed to read sysfs attribute: {0}")]
    SysfsRead(#[source] std::io::Error),

    #[error("failed to write sysfs attribute: {0}")]
    SysfsWrite(#[source] std::io::Error),

    #[error("sysfs returned an unexpected value: {0:?}")]
    UnexpectedSysfsValue(String),

    #[error("not authorized to perform this action")]
    NotAuthorized,

    #[error("could not reach polkit authority: {0}")]
    PolkitUnavailable(#[source] zbus::Error),
}

/// Maps our internal errors onto standard D-Bus error names a client can
/// branch on, instead of leaking raw paths or `io::Error` internals across
/// the bus.
impl From<ControlError> for zbus::fdo::Error {
    fn from(err: ControlError) -> Self {
        match err {
            ControlError::NotAuthorized => zbus::fdo::Error::AccessDenied(err.to_string()),
            ControlError::OutOfRange { .. } => zbus::fdo::Error::InvalidArgs(err.to_string()),
            ControlError::Unsupported => zbus::fdo::Error::NotSupported(err.to_string()),
            ControlError::SysfsRead(_)
            | ControlError::SysfsWrite(_)
            | ControlError::UnexpectedSysfsValue(_)
            | ControlError::PolkitUnavailable(_) => zbus::fdo::Error::Failed(err.to_string()),
        }
    }
}
