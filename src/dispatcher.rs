use std::process::Command;
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
        
        // Spawn the command asynchronously so we don't block the dispatcher
        tokio::spawn(async move {
            let mut parts = cmd.split_whitespace();
            if let Some(program) = parts.next() {
                let args: Vec<&str> = parts.collect();
                
                match Command::new(program).args(&args).output() {
                    Ok(output) => {
                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            error!("Command failed ({}): {}", program, stderr);
                        } else {
                            debug!("Command success: {}", program);
                        }
                    }
                    Err(e) => {
                        error!("Failed to execute command {}: {}", program, e);
                    }
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
                        {
                            error!("Failed to send OSD notification: {}", e);
                        }
                    });
                } else if osd_config.backend == "pipe" {
                    if let Some(ref path) = osd_config.pipe_path {
                        let path_clone = path.clone();
                        tokio::spawn(async move {
                            if let Err(e) = std::fs::write(&path_clone, format!("{}\n", msg)) {
                                error!("Failed to write to OSD pipe {}: {}", path_clone, e);
                            }
                        });
                    }
                }
            }
        }
    }
}
