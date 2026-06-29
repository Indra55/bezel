use tokio::process::Command;
use tracing::{debug, error, info};

use crate::gesture::ActionCommand;
use crate::config::OsdConfig;

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
                Err(e) => {
                    error!("Failed to execute command {}: {}", cmd, e);
                }
            }
        });

        // Handle OSD if enabled
        if osd_config.enabled {
            if let Some(msg) = action.osd_message {
                if osd_config.backend == "notify-send" {
                    tokio::spawn(async move {
                        if let Err(e) = Command::new("notify-send")
                            .arg("-t")
                            .arg("1000")
                            .arg("Bezel")
                            .arg(&msg)
                            .output()
                            .await
                        {
                            error!("Failed to send OSD notification: {}", e);
                        }
                    });
                } else if osd_config.backend == "pipe" {
                    if let Some(ref path) = osd_config.pipe_path {
                        let path_clone = path.clone();
                        tokio::spawn(async move {
                            if let Err(e) = tokio::fs::write(&path_clone, format!("{}\n", msg)).await {
                                error!("Failed to write to OSD pipe {}: {}", path_clone, e);
                            }
                        });
                    }
                }
            }
        }
    }
}
