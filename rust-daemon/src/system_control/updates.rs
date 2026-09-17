//! OS-update logic. No D-Bus here -- mirrors `battery.rs`/`platform_profile.rs`.
//! `apply_os` doesn't run `apt` itself; the daemon's own unit is sandboxed
//! (`ProtectSystem=strict`) away from writing to most of the filesystem, so
//! it starts the separate, unsandboxed `controlcenterd-updates@os.service`
//! and waits for it.

use tokio::process::Command;

use crate::error::ControlError;

const APPLY_UNIT: &str = "controlcenterd-updates@os.service";
const APPLY_LOG: &str = "/run/controlcenterd/update-os.log";

/// Read-only, no polkit gate needed.
pub async fn preview_os() -> Result<String, ControlError> {
    let output = run("apt", &["list", "--upgradable"]).await?;
    if !output.status.success() {
        return Err(ControlError::UpdateFailed {
            category: "os preview".to_string(),
            status: output.status.to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn apply_os() -> Result<(), ControlError> {
    let output = run("systemctl", &["start", "--wait", APPLY_UNIT]).await?;

    if output.status.success() {
        Ok(())
    } else {
        // Log read is best-effort; fall back to the bare exit status.
        let detail = tokio::fs::read_to_string(APPLY_LOG)
            .await
            .map(|log| tail(&log, 20))
            .unwrap_or_else(|_| output.status.to_string());

        Err(ControlError::UpdateFailed {
            category: "os".to_string(),
            status: detail,
        })
    }
}

fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

async fn run(program: &str, args: &[&str]) -> Result<std::process::Output, ControlError> {
    Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|source| ControlError::CommandSpawn {
            command: format!("{program} {}", args.join(" ")),
            source,
        })
}
