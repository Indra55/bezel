use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use crate::config::OsdConfig;
use crate::focus::{Compositor, Focus};
use crate::gesture::ActionCommand;

pub async fn run_dispatcher(
    mut rx: tokio::sync::mpsc::Receiver<ActionCommand>,
    osd_config: OsdConfig,
    compositor: Option<Compositor>,
    focus: watch::Receiver<Focus>,
) {
    info!("Action dispatcher started");
    let osd_config = Arc::new(osd_config);
    let compositor = compositor.map(Arc::new);

    while let Some(action) = rx.recv().await {
        let osd_config = osd_config.clone();
        let compositor = compositor.clone();
        let focus = focus.clone();
        tokio::spawn(async move {
            let cmd = action.cmd;
            let mut command = Command::new("sh");
            command.arg("-c").arg(&cmd);
            if let Some(target) = &action.target {
                let Some(compositor) = compositor else { return };
                match compositor.verify_focus(&focus, target).await {
                    Ok(true) => {}
                    Ok(false) => {
                        debug!("Skipping app gesture because the target is no longer focused");
                        return;
                    }
                    Err(err) => {
                        debug!(%err, "Skipping app gesture because focus could not be verified");
                        return;
                    }
                }
                command
                    .env("BEZEL_APP_ID", &target.app_id)
                    .env("BEZEL_WINDOW_ID", &target.id);
            }
            info!("Executing action: {}", cmd);
            let output = command.output();
            if osd_config.enabled {
                if let Some(msg) = action.osd_message {
                    send_osd(&osd_config, msg);
                }
            }
            match output.await {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        error!("Command failed ({}): {}", cmd, stderr);
                    } else {
                        debug!("Command success: {}", cmd);
                    }
                }
                Err(e) => error!("Failed to execute command {}: {}", cmd, e),
            }
        });
    }
}

fn send_osd(osd_config: &OsdConfig, msg: String) {
    match osd_config.backend.as_str() {
        "notify-send" => {
            let use_hints = osd_config.canonical_hints.unwrap_or(false);
            tokio::spawn(async move {
                let mut c = Command::new("notify-send");
                c.arg("-a")
                    .arg("Bezel")
                    .arg("-i")
                    .arg("input-touchpad")
                    .arg("-t")
                    .arg("1000")
                    .arg("-e")
                    .arg("Bezel")
                    .arg(&msg);

                if use_hints {
                    c.arg("-h")
                        .arg("string:x-canonical-private-synchronous:bezel")
                        .arg("-h")
                        .arg("int:transient:1");
                }

                match c.output().await {
                    Ok(output) => {
                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            warn!(
                                "notify-send exited {:?}: {}",
                                output.status.code(),
                                stderr.trim()
                            );
                        }
                    }
                    Err(e) => error!("Failed to spawn notify-send: {}", e),
                }
            });
        }

        "swayosd" => {
            tokio::spawn(async move {
                match Command::new("swayosd-client")
                    .arg("--custom-message")
                    .arg(&msg)
                    .output()
                    .await
                {
                    Ok(output) if !output.status.success() => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        warn!("swayosd-client failed: {}", stderr.trim());
                    }
                    Err(e) => error!("Failed to spawn swayosd-client: {}", e),
                    _ => {}
                }
            });
        }

        "pipe" => {
            if let Some(ref path) = osd_config.pipe_path {
                let path_clone = path.clone();
                tokio::spawn(async move {
                    if let Err(e) = tokio::fs::write(&path_clone, format!("{}\n", msg)).await {
                        error!("Failed to write to OSD pipe {}: {}", path_clone, e);
                    }
                });
            } else {
                warn!("OSD backend is 'pipe' but pipe_path is not set");
            }
        }

        other => warn!(
            "Unknown OSD backend: '{}'. Valid: notify-send, swayosd, pipe",
            other
        ),
    }
}
