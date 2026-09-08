mod battery;
mod dbus_iface;
mod error;
mod polkit;

use dbus_iface::BatteryInterface;

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
        // Not a hard failure: it lets you run the binary directly during
        // development to see it register on the bus and respond to
        // GetChargeLimit. Any actual SetChargeLimit write will fail with a
        // permission error until this runs as root.
        tracing::warn!(
            "not running as root -- hardware writes to sysfs will fail with permission errors"
        );
    }

    let threshold_path = battery::discover_threshold_path();
    match &threshold_path {
        Some(path) => {
            tracing::info!(path = %path.display(), "found battery charge-limit control")
        }
        None => tracing::warn!(
            "no battery on this system exposes a charge-limit control; \
             the Battery interface will report Supported=false"
        ),
    }

    let battery_iface = BatteryInterface::new(threshold_path);

    let connection = zbus::connection::Builder::system()?
        .name(BUS_NAME)?
        .serve_at(OBJECT_PATH, battery_iface)?
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
    // No extra crate for this -- /proc/self/status is always available on
    // Linux and its "Uid:" line lists real/effective/saved/filesystem uid.
    // We care about the effective uid (2nd field), since that's what
    // governs whether our sysfs writes are actually permitted.
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
