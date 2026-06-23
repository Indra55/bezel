use anyhow::{bail, Context, Result};
use evdev::{Device, InputEventKind, AbsoluteAxisType, InputEvent};
use tracing::{debug, error, info};
use std::time::Instant;

use crate::config::Config;
use crate::gesture::{classify_gesture, GestureEvent, Zone};
use crate::passthrough::create_virtual_device;

const SLOT_COUNT: usize = 10;

#[derive(Debug, Clone, Default)]
pub struct SlotState {
    pub active: bool,
    pub claimed: bool,
    pub tracking_id: i32,
    pub start_x: Option<f32>,
    pub start_y: Option<f32>,
    pub current_x: f32,
    pub current_y: f32,
    pub start_time: Option<Instant>,
    pub assigned_zone: Option<Zone>,
    pub needs_zone_check: bool,
}

pub fn find_trackpad() -> Result<Device> {
    for entry in std::fs::read_dir("/dev/input").context("Failed to read /dev/input")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        if let Ok(device) = Device::open(&path) {
            if let Some(abs_axes) = device.supported_absolute_axes() {
                if abs_axes.contains(AbsoluteAxisType::ABS_X)
                    && abs_axes.contains(AbsoluteAxisType::ABS_Y)
                    && abs_axes.contains(AbsoluteAxisType::ABS_MT_POSITION_X)
                    && abs_axes.contains(AbsoluteAxisType::ABS_MT_POSITION_Y)
                {
                    info!(
                        "Found trackpad automatically: {} at {:?}",
                        device.name().unwrap_or("Unknown"),
                        path
                    );
                    return Ok(device);
                }
            }
        }
    }
    bail!("No trackpad device found automatically. Check your /dev/input/ permissions.");
}

fn determine_zone(norm_x: f32, norm_y: f32, config: &Config) -> Option<Zone> {
    if norm_x < config.zones.left_width {
        Some(Zone::Left)
    } else if norm_x > 1.0 - config.zones.right_width {
        Some(Zone::Right)
    } else if norm_y < config.zones.top_height {
        Some(Zone::Top)
    } else if norm_y > 1.0 - config.zones.bottom_height {
        Some(Zone::Bottom)
    } else {
        None
    }
}

pub async fn run_input_reader(
    config_rx: tokio::sync::watch::Receiver<Config>,
    gesture_tx: tokio::sync::mpsc::Sender<GestureEvent>,
) -> Result<()> {
    // Initial config for device setup
    let config = config_rx.borrow().clone();

    let mut device = if config.device.path == "auto" {
        find_trackpad()?
    } else {
        Device::open(&config.device.path)
            .with_context(|| format!("Failed to open device at {}", config.device.path))?
    };

    if let Err(e) = device.grab() {
        error!("Failed to grab device exclusively. Ensure you have permissions or the device isn't grabbed by another process.");
        error!("Hint: Add yourself to the `input` group and ensure correct udev rules are set.");
        return Err(e.into());
    }
    info!("Successfully grabbed device.");

    let mut virtual_device = create_virtual_device(&device)?;

    let abs_state = device.get_abs_state().context("Failed to get abs state")?;
    let x_info = abs_state[AbsoluteAxisType::ABS_MT_POSITION_X.0 as usize];
    let y_info = abs_state[AbsoluteAxisType::ABS_MT_POSITION_Y.0 as usize];

    let x_min = x_info.minimum as f32;
    let x_max = x_info.maximum as f32;
    let x_range = x_max - x_min;

    let y_min = y_info.minimum as f32;
    let y_max = y_info.maximum as f32;
    let y_range = y_max - y_min;

    info!("Trackpad bounds: X({} - {}), Y({} - {})", x_min, x_max, y_min, y_max);

    // We will spawn a blocking task to read from the device since fetch_events is blocking.
    tokio::task::spawn_blocking(move || {
        let mut slots: [SlotState; SLOT_COUNT] = Default::default();
        let mut current_slot: usize = 0;
        let mut active_fingers: u8 = 0;

        loop {
            let mut pending_passthrough = Vec::new();
            let mut sync_needed = false;

            match device.fetch_events() {
                Ok(events) => {
                    for ev in events {
                        let mut forward_this_event = true;
                        sync_needed = true;

                        match ev.kind() {
                            InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_SLOT) => {
                                current_slot = ev.value() as usize;
                                if current_slot >= SLOT_COUNT {
                                    current_slot = SLOT_COUNT - 1;
                                }
                            }
                            InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_TRACKING_ID) => {
                                if ev.value() == -1 {
                                    // Finger lifted
                                    if slots[current_slot].active {
                                        if slots[current_slot].claimed {
                                            let s = &slots[current_slot];
                                            if let (Some(sx), Some(sy)) = (s.start_x, s.start_y) {
                                                let norm_dx = (s.current_x - sx) / x_range;
                                                let norm_dy = (s.current_y - sy) / y_range;
                                                let duration = s.start_time.unwrap_or_else(|| Instant::now()).elapsed().as_millis();

                                                if let Some(zone) = s.assigned_zone {
                                                    if let Some(gesture) = classify_gesture(zone, norm_dx, norm_dy, active_fingers, duration) {
                                                        let _ = gesture_tx.blocking_send(gesture);
                                                    }
                                                }
                                            }
                                            forward_this_event = false;
                                        }
                                        slots[current_slot] = Default::default();
                                        if active_fingers > 0 {
                                            active_fingers -= 1;
                                        }
                                    }
                                } else {
                                    // New finger down
                                    slots[current_slot].active = true;
                                    slots[current_slot].claimed = false;
                                    slots[current_slot].tracking_id = ev.value();
                                    slots[current_slot].start_time = Some(Instant::now());
                                    slots[current_slot].needs_zone_check = true;
                                    active_fingers += 1;
                                }
                            }
                            InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_POSITION_X) => {
                                let val = ev.value() as f32;
                                slots[current_slot].current_x = val;
                                
                                if slots[current_slot].start_x.is_none() {
                                    slots[current_slot].start_x = Some(val);
                                }
                                
                                if slots[current_slot].claimed {
                                    forward_this_event = false;
                                }
                            }
                            InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_POSITION_Y) => {
                                let val = ev.value() as f32;
                                slots[current_slot].current_y = val;
                                
                                if slots[current_slot].start_y.is_none() {
                                    slots[current_slot].start_y = Some(val);
                                }
                                
                                if slots[current_slot].claimed {
                                    forward_this_event = false;
                                }
                            }
                            InputEventKind::Synchronization(_) => {
                                // Handled below
                            }
                            _ => {}
                        }

                        if forward_this_event {
                            pending_passthrough.push(ev);
                        }
                    }

                    if sync_needed {
                        // Before emitting, check if any active slots need zone classification
                        for i in 0..SLOT_COUNT {
                            if slots[i].active && slots[i].needs_zone_check {
                                if let (Some(sx), Some(sy)) = (slots[i].start_x, slots[i].start_y) {
                                    let norm_x = (sx - x_min) / x_range;
                                    let norm_y = (sy - y_min) / y_range;
                                    
                                    let current_config = config_rx.borrow().clone();
                                    if let Some(zone) = determine_zone(norm_x, norm_y, &current_config) {
                                        slots[i].claimed = true;
                                        slots[i].assigned_zone = Some(zone);
                                        debug!("Touch in slot {} claimed in zone {:?}", i, zone);
                                        
                                        // We should also remove the events for this slot from pending_passthrough
                                        // since we just claimed it, but it might have already been added.
                                        // For simplicity, it will just start dropping future events.
                                    }
                                    slots[i].needs_zone_check = false;
                                }
                            }
                        }

                        if !pending_passthrough.is_empty() {
                            pending_passthrough.push(InputEvent::new(evdev::EventType::SYNCHRONIZATION, evdev::Synchronization::SYN_REPORT.0, 0));
                            if let Err(e) = virtual_device.emit(&pending_passthrough) {
                                error!("Failed to write to virtual device: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading events: {}. Device might be disconnected.", e);
                    break;
                }
            }
        }
    });

    Ok(())
}
