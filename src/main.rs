use anyhow::Result;
use notify::{Event, RecursiveMode, Watcher};
use tokio::sync::{mpsc, watch};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

mod cli;
mod config;
mod device;
mod dispatcher;
mod gesture;
mod passthrough;
mod status;

use config::load_config;
use device::run_input_reader;
use dispatcher::run_dispatcher;
use gesture::{ActionCommand, Gesture, GestureEvent};

#[tokio::main]
async fn main() -> Result<()> {
    let Some((config_path, initial_config)) = cli::handle_args()? else {
        return Ok(());
    };

    let filter = EnvFilter::try_from_env("BEZEL_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Starting Bezel daemon");
    info!("Wayland users: Please configure your compositor to apply your preferred trackpad settings (like tap-to-click) to the 'Bezel Virtual Trackpad' device.");

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
    let reload_path = config_path.clone();
    let mut watcher =
        notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
            Ok(event) => {
                if event.kind.is_modify() {
                    info!("Config file modified, reloading...");
                    match load_config(&reload_path) {
                        Ok(new_config) => {
                            let _ = config_tx.send(new_config);
                        }
                        Err(e) => error!("Failed to reload config: {}", e),
                    }
                }
            }
            Err(e) => error!("watch error: {:?}", e),
        })?;

    if config_path.exists() {
        if let Some(parent) = config_path.parent() {
            watcher.watch(parent, RecursiveMode::NonRecursive)?;
        }
    } else {
        info!("Config file not found. Defaulting built-in configuration.");
    }

    while let Some(event) = gesture_rx.recv().await {
        info!(
            "Matched {:?} in {:?} (fingers: {}, magnitude: {:.2}) to command: {}",
            event.gesture, event.zone, event.finger_count, event.magnitude, event.action.cmd
        );
        let name = format!("{:?}", event.gesture);
        let message = if event.finger_count == 1
            && matches!(
                event.gesture,
                Gesture::Tap
                    | Gesture::SwipeUp
                    | Gesture::SwipeDown
                    | Gesture::SwipeLeft
                    | Gesture::SwipeRight
            ) {
            // Preserve the original OSD pipe messages consumed by the visualizer.
            format!("{:?} {}", event.zone, name.trim_start_matches("Swipe"))
        } else {
            format!("{:?} {} ({} fingers)", event.zone, name, event.finger_count)
        };
        let cmd = ActionCommand {
            cmd: event.action.cmd,
            osd_message: Some(message),
        };
        let _ = action_tx.send(cmd).await;
    }
    Ok(())
}
