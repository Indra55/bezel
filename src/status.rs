use anyhow::{ensure, Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::config::load_config;

pub fn control(action: &str) -> Result<()> {
    let output = Command::new("systemctl")
        .args(["--user", action, "bezel.service"])
        .output()
        .with_context(|| format!("Could not {action} bezel.service; systemctl is required"))?;
    ensure!(
        output.status.success(),
        "Could not {action} bezel.service: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let result = if action == "restart" {
        "restarted"
    } else {
        "stopped"
    };
    println!("Service {result}: bezel.service");
    Ok(())
}

pub fn show(config_path: &Path) -> Result<()> {
    let output = Command::new("systemctl")
        .args([
            "--user",
            "show",
            "bezel.service",
            "--no-pager",
            "--property=LoadState,ActiveState,SubState,MainPID",
        ])
        .output()
        .context("Could not query bezel.service; systemctl is required for --status")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let properties: HashMap<_, _> = stdout
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect();
    let value = |name| properties.get(name).copied().unwrap_or("unknown");
    ensure!(
        output.status.success() || value("LoadState") == "not-found",
        "Could not query bezel.service: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    if value("LoadState") == "not-found" {
        println!("Service: bezel.service is not installed");
    } else {
        println!("Service: {} ({})", value("ActiveState"), value("SubState"));
    }
    if let Ok(pid) = value("MainPID").parse::<u32>() {
        if pid > 0 {
            println!("Service PID: {pid}");
            match std::fs::read_dir(format!("/proc/{pid}/fd")) {
                Ok(entries) => {
                    let mut devices: Vec<_> = entries
                        .filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
                        .filter(|path| path.starts_with("/dev/input"))
                        .collect();
                    devices.sort();
                    devices.dedup();
                    if devices.is_empty() {
                        println!("Open input device: none readable");
                    }
                    for device in devices {
                        println!("Open input device: {}", device.display());
                    }
                }
                Err(error) => println!("Open input device: unavailable ({error})"),
            }
        }
    }
    println!("Config inspected: {}", config_path.display());
    if config_path.try_exists()? {
        let config = load_config(config_path)?;
        println!("Configured device: {}", config.device.path);
        println!("Configuration: valid");
    } else {
        println!("Configuration: missing (default startup uses built-in settings)");
    }
    Ok(())
}
