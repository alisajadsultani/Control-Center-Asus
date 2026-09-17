use thiserror::Error;

/// Domain errors for the daemon. The `From<ControlError> for
/// zbus::fdo::Error` impl below is the one place that decides what a D-Bus
/// caller actually sees -- never a raw `io::Error` or a path.
#[derive(Debug, Error)]
pub enum ControlError {
    #[error("no battery on this system exposes a charge-limit control")]
    Unsupported,

    #[error("charge limit {value} is out of the supported range {min}-{max}")]
    OutOfRange { value: u8, min: u8, max: u8 },

    #[error("platform profile {value:?} is not supported; choices are {choices:?}")]
    InvalidProfile { value: String, choices: Vec<String> },

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

    #[error("failed to run `{command}`: {source}")]
    CommandSpawn {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("update task for {category} failed ({status})")]
    UpdateFailed { category: String, status: String },
}

impl From<ControlError> for zbus::fdo::Error {
    fn from(err: ControlError) -> Self {
        match err {
            ControlError::NotAuthorized => zbus::fdo::Error::AccessDenied(err.to_string()),
            ControlError::OutOfRange { .. } | ControlError::InvalidProfile { .. } => {
                zbus::fdo::Error::InvalidArgs(err.to_string())
            }
            ControlError::Unsupported => zbus::fdo::Error::NotSupported(err.to_string()),
            ControlError::SysfsRead(_)
            | ControlError::SysfsWrite(_)
            | ControlError::UnexpectedSysfsValue(_)
            | ControlError::PolkitUnavailable(_)
            | ControlError::CommandSpawn { .. }
            | ControlError::UpdateFailed { .. } => zbus::fdo::Error::Failed(err.to_string()),
        }
    }
}
