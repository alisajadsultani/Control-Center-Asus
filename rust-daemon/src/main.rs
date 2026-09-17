mod error;
mod polkit;
mod system_control;

use system_control::{
    discover_platform_path, discover_threshold_path, BatteryInterface, PlatformProfileInterface,
    UpdatesInterface,
};

/// D-Bus well-known name this daemon owns on the system bus.
const BUS_NAME: &str = "org.controlcenter.Daemon1";
/// Object path the Battery interface is served at.
const OBJECT_PATH: &str = "/org/controlcenter/Daemon1";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    if !running_as_root() {
        // Not fatal: lets the binary run during development to see it
        // register on the bus; SetChargeLimit will fail until run as root.
        tracing::warn!(
            "not running as root -- hardware writes to sysfs will fail with permission errors"
        );
    }

    let threshold_path = discover_threshold_path();
    match &threshold_path {
        Some(path) => {
            tracing::info!(path = %path.display(), "found battery charge-limit control")
        }
        None => tracing::warn!(
            "no battery on this system exposes a charge-limit control; \
             the Battery interface will report Supported=false"
        ),
    }

    let platform_path  = discover_platform_path();
    match &platform_path {
        Some(path) => {
            tracing::info!(path = %path.display(), "found platform profile control")
        }
        None => tracing::warn!{
            "no platform profile on this system exposes a control; \
            the platform profile will report Supported=false"
        },
    }

    let battery_iface = BatteryInterface::new(threshold_path);
    let platform_profile_iface = PlatformProfileInterface::new(platform_path);
    let updates_iface = UpdatesInterface::default();

    let connection = zbus::connection::Builder::system()?
        .name(BUS_NAME)?
        .serve_at(OBJECT_PATH, battery_iface)?
        .serve_at(OBJECT_PATH, platform_profile_iface)?
        .serve_at(OBJECT_PATH, updates_iface)?
        .build()
        .await?;

    tracing::info!(
        bus_name = BUS_NAME,
        object_path = OBJECT_PATH,
        "controlcenterd ready"
    );

    wait_for_shutdown_signal().await;
    tracing::info!("shutting down");

    drop(connection);
    Ok(())
}

fn running_as_root() -> bool {
    // /proc/self/status's "Uid:" line lists real/effective/saved/fs uid;
    // the effective uid (2nd field) is what governs sysfs write permission.
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return false;
    };
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|rest| rest.split_whitespace().nth(1))
        .and_then(|euid| euid.parse::<u32>().ok())
        == Some(0)
}

async fn wait_for_shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigterm =
        signal(SignalKind::terminate()).expect("failed to register SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to register SIGINT handler");
    tokio::select! {
        _ = sigterm.recv() => tracing::info!("received SIGTERM"),
        _ = sigint.recv() => tracing::info!("received SIGINT"),
    }
}
