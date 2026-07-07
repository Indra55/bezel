use anyhow::Result;
use notify::{Event, RecursiveMode, Watcher};
use tokio::sync::{mpsc, watch};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

mod config;
mod device;
mod dispatcher;
mod gesture;
mod passthrough;

use config::{get_config_path, load_config};
use device::run_input_reader;
use dispatcher::run_dispatcher;
use gesture::{ActionCommand, GestureEvent};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && (args[1] == "--version" || args[1] == "-V") {
        println!("bezel v{}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    let filter = EnvFilter::try_from_env("BEZEL_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Starting Bezel daemon");
    info!("Wayland users: Please configure your compositor to apply your preferred trackpad settings (like tap-to-click) to the 'Bezel Virtual Trackpad' device.");

    let initial_config = load_config()?;
    let (config_tx, config_rx) = watch::channel(initial_config.clone());

    let (gesture_tx, mut gesture_rx) = mpsc::channel::<GestureEvent>(64); // Input reader bursts
    let (action_tx, action_rx) = mpsc::channel::<ActionCommand>(32); // Lower volume

    let config_rx_for_reader = config_rx.clone();
    tokio::spawn(async move {
        if let Err(e) = run_input_reader(config_rx_for_reader, gesture_tx).await {
            error!("Input reader error: {:?}", e);
        }
    });

    let osd_config = initial_config.osd.clone();
    tokio::spawn(async move {
        run_dispatcher(action_rx, osd_config).await;
    });

    // Setup hot-reloading for config
    let mut watcher =
        notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
            Ok(event) => {
                if event.kind.is_modify() {
                    info!("Config file modified, reloading...");
                    match load_config() {
                        Ok(new_config) => {
                            let _ = config_tx.send(new_config);
                        }
                        Err(e) => error!("Failed to reload config: {}", e),
                    }
                }
            }
            Err(e) => error!("watch error: {:?}", e),
        })?;

    let config_path = get_config_path();
    if config_path.exists() {
        if let Some(parent) = config_path.parent() {
            watcher.watch(parent, RecursiveMode::NonRecursive)?;
        }
    } else {
        info!("Config file not found. Defaulting built-in configuration.");
    }

    while let Some(event) = gesture_rx.recv().await {
        let current_config = config_rx.borrow().clone();

        if let Some(zone_gestures) = current_config.gestures.get(&event.zone) {
            if let Some(gesture_action) = zone_gestures.get(&event.direction) {
                if gesture_action.action == "command" {
                    info!(
                        "Matched gesture {:?} in zone {:?} (fingers: {}, mag: {:.2}) to command: {}",
                        event.direction, event.zone, event.finger_count, event.magnitude, gesture_action.cmd
                    );
                    let cmd = ActionCommand {
                        cmd: gesture_action.cmd.clone(),
                        osd_message: Some(format!("{:?} {:?}", event.zone, event.direction)),
                    };
                    let _ = action_tx.send(cmd).await;
                }
            }
        }
    }

    Ok(())
}
