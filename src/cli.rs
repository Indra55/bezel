use anyhow::{bail, ensure, Context, Result};
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use crate::config::{get_config_path, load_config, Config};

const HELP: &str = "Usage: bezel [--config PATH] [OPTION]

Run without an action option to start the gesture daemon.

Options:
  --configure [OPTIONS]  Open Gesture Studio to edit your gesture bindings
  --check-config         Validate the selected configuration without starting
  --list-devices         List readable, compatible trackpads without grabbing them
  --config PATH          Use this file for startup, reloads, validation, or editing
  --status               Show the user service, open input device, and config inspected
  -h, --help             Show this help
  -V, --version          Show the version

Gesture Studio options (after --configure):
  --plain               Use numbered prompts
  --no-animation        Disable gesture preview animation
  --color, --no-color    Enable or disable color
  --desktop NAME        Use hyprland, niri, sway, plasma, gnome, or generic
  --nixos               Print a Home Manager configuration
  --toml                Save a TOML configuration instead
  --if-missing          Keep an existing TOML configuration unchanged

Example: bezel --config ./gestures.toml --configure --plain";

pub fn handle_args() -> Result<Option<(PathBuf, Config)>> {
    let mut args = std::env::args_os().skip(1);
    let mut config_path = None;
    let mut action = None;
    while let Some(option) = args.next() {
        match option.to_str() {
            Some("--config") => {
                ensure!(config_path.is_none(), "--config may only be supplied once");
                let path = args.next().context("--config needs a path")?;
                ensure!(!path.is_empty(), "--config needs a path");
                config_path = Some(std::env::current_dir()?.join(path));
            }
            Some("--configure") => {
                ensure!(
                    action.is_none(),
                    "Choose one action. Run bezel --help for usage."
                );
                let rest: Vec<_> = args.collect();
                if rest.len() == 1 && (rest[0] == "--help" || rest[0] == "-h") {
                    println!("{HELP}");
                    return Ok(None);
                }
                return configure(config_path, rest);
            }
            Some("--check-config" | "--list-devices" | "--status") => {
                ensure!(
                    action.is_none(),
                    "Choose one action. Run bezel --help for usage."
                );
                action = option.to_str().map(str::to_owned);
            }
            Some("--help" | "-h" | "--version" | "-V") => {
                ensure!(
                    action.is_none() && args.next().is_none(),
                    "Invalid arguments. Run bezel --help for usage."
                );
                if option == "--help" || option == "-h" {
                    println!("{HELP}");
                } else {
                    println!("bezel v{}", env!("CARGO_PKG_VERSION"));
                }
                return Ok(None);
            }
            _ => bail!(
                "Unexpected argument: {}. Run bezel --help for usage.",
                option.to_string_lossy()
            ),
        }
    }

    let explicit = config_path.is_some();
    let path = config_path.unwrap_or_else(get_config_path);
    match action.as_deref() {
        Some("--check-config") => {
            load_config(&path)?;
            println!("Configuration valid: {}", path.display());
        }
        Some("--list-devices") => {
            ensure!(!explicit, "--config cannot be used with --list-devices");
            crate::device::list_trackpads()?;
        }
        Some("--status") => crate::status::show(&path)?,
        None => {
            let config = if explicit || path.try_exists()? {
                load_config(&path)?
            } else {
                eprintln!(
                    "Config file not found at {}; using built-in defaults",
                    path.display()
                );
                Config::default()
            };
            return Ok(Some((path, config)));
        }
        _ => unreachable!(),
    }
    Ok(None)
}

fn configure(
    config_path: Option<PathBuf>,
    args: Vec<OsString>,
) -> Result<Option<(PathBuf, Config)>> {
    let helper = std::env::current_exe()?.with_file_name("bezel-onboard");
    let mut command = if helper.is_file() {
        Command::new(helper)
    } else {
        let mut command = Command::new("bash");
        command.args(["-c", include_str!("../onboard.sh"), "bezel-onboard"]);
        command
    };
    if let Some(path) = config_path {
        command.arg("--config").arg(path);
    }
    let error = command.args(args).exec();
    Err(error).context("Could not open Gesture Studio (Bash is required)")
}
