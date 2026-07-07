use tokio::process::Command;
use tracing::{debug, error, info, warn};

use crate::config::OsdConfig;
use crate::gesture::ActionCommand;

pub async fn run_dispatcher(
    mut rx: tokio::sync::mpsc::Receiver<ActionCommand>,
    osd_config: OsdConfig,
) {
    info!("Action dispatcher started");

    while let Some(action) = rx.recv().await {
        info!("Executing action: {}", action.cmd);

        let cmd = action.cmd.clone();

        tokio::spawn(async move {
            match Command::new("sh").arg("-c").arg(&cmd).output().await {
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

        if osd_config.enabled {
            if let Some(msg) = action.osd_message {
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
                                if let Err(e) =
                                    tokio::fs::write(&path_clone, format!("{}\n", msg)).await
                                {
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
        }
    }
}
