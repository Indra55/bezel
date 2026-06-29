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

/// Key codes for BTN_TOOL_* events used in single-touch synthesis.
const BTN_TOUCH: u16 = 0x14a;
const BTN_TOOL_FINGER: u16 = 0x145;
const BTN_TOOL_DOUBLETAP: u16 = 0x14d;
const BTN_TOOL_TRIPLETAP: u16 = 0x14e;
const BTN_TOOL_QUADTAP: u16 = 0x14f;
const BTN_TOOL_QUINTTAP: u16 = 0x148;

/// Event types/codes we need to filter and synthesize.
fn is_single_touch_emulation(ev: &InputEvent) -> bool {
    match ev.kind() {
        InputEventKind::AbsAxis(AbsoluteAxisType::ABS_X) => true,
        InputEventKind::AbsAxis(AbsoluteAxisType::ABS_Y) => true,
        InputEventKind::Key(key) => {
            let code = key.code();
            code == BTN_TOUCH
                || code == BTN_TOOL_FINGER
                || code == BTN_TOOL_DOUBLETAP
                || code == BTN_TOOL_TRIPLETAP
                || code == BTN_TOOL_QUADTAP
                || code == BTN_TOOL_QUINTTAP
        }
        _ => false,
    }
}

/// Returns the BTN_TOOL_* code for a given finger count, if any.
fn btn_tool_for_count(count: u8) -> Option<u16> {
    match count {
        1 => Some(BTN_TOOL_FINGER),
        2 => Some(BTN_TOOL_DOUBLETAP),
        3 => Some(BTN_TOOL_TRIPLETAP),
        4 => Some(BTN_TOOL_QUADTAP),
        5 => Some(BTN_TOOL_QUINTTAP),
        _ => None,
    }
}

/// Find the lowest-numbered active, unclaimed slot to use as the primary
/// source for ABS_X/ABS_Y single-touch coordinates.
fn find_primary_unclaimed(slots: &[SlotState; SLOT_COUNT]) -> Option<usize> {
    (0..SLOT_COUNT).find(|&i| slots[i].active && !slots[i].claimed)
}

/// Count unclaimed (active and not claimed) fingers.
fn count_unclaimed(slots: &[SlotState; SLOT_COUNT]) -> u8 {
    slots.iter().filter(|s| s.active && !s.claimed).count() as u8
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

        let mut prev_unclaimed_count: u8 = 0;
        let mut prev_btn_touch: bool = false;

        let mut frame_events: Vec<InputEvent> = Vec::with_capacity(64);

        loop {
            match device.fetch_events() {
                Ok(events) => {
                    for ev in events {
                        if ev.kind() == InputEventKind::Synchronization(evdev::Synchronization::SYN_DROPPED) {
                            error!("evdev buffer overflow (SYN_DROPPED). Resetting tracking state to prevent freeze.");
                            slots = Default::default();
                            active_fingers = 0;
                            frame_events.clear();
                            
                            // Send a quick reset to the virtual device to release any stuck touches
                            if prev_btn_touch {
                                let mut output_events = Vec::new();
                                if let Some(old_code) = btn_tool_for_count(prev_unclaimed_count) {
                                    output_events.push(InputEvent::new(
                                        evdev::EventType::KEY, old_code, 0,
                                    ));
                                }
                                output_events.push(InputEvent::new(
                                    evdev::EventType::KEY, BTN_TOUCH, 0,
                                ));
                                output_events.push(InputEvent::new(
                                    evdev::EventType::SYNCHRONIZATION,
                                    evdev::Synchronization::SYN_REPORT.0,
                                    0,
                                ));
                                if let Err(e) = virtual_device.emit(&output_events) {
                                    error!("Failed to recover virtual device: {}", e);
                                }
                                prev_btn_touch = false;
                                prev_unclaimed_count = 0;
                            }
                            continue;
                        }

                        if ev.kind() == InputEventKind::Synchronization(evdev::Synchronization::SYN_REPORT) {
                            let entry_slot = current_slot;
                            let mut frame_slot = current_slot;

                            for fev in &frame_events {
                                match fev.kind() {
                                    InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_SLOT) => {
                                        frame_slot = fev.value() as usize;
                                        if frame_slot >= SLOT_COUNT {
                                            frame_slot = SLOT_COUNT - 1;
                                        }
                                    }
                                    InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_TRACKING_ID) => {
                                        if fev.value() == -1 {
                                            if slots[frame_slot].active {
                                                if slots[frame_slot].claimed {
                                                    let s = &slots[frame_slot];
                                                    if let (Some(sx), Some(sy)) = (s.start_x, s.start_y) {
                                                        let norm_dx = (s.current_x - sx) / x_range;
                                                        let norm_dy = (s.current_y - sy) / y_range;
                                                        let duration = s.start_time
                                                            .unwrap_or_else(|| Instant::now())
                                                            .elapsed()
                                                            .as_millis();

                                                        if let Some(zone) = s.assigned_zone {
                                                            if let Some(gesture) = classify_gesture(
                                                                zone, norm_dx, norm_dy,
                                                                active_fingers, duration,
                                                            ) {
                                                                let _ = gesture_tx.blocking_send(gesture);
                                                            }
                                                        }
                                                    }
                                                }
                                                slots[frame_slot] = Default::default();
                                                if active_fingers > 0 {
                                                    active_fingers -= 1;
                                                }
                                            }
                                        } else {
                                            slots[frame_slot].active = true;
                                            slots[frame_slot].claimed = false;
                                            slots[frame_slot].tracking_id = fev.value();
                                            slots[frame_slot].start_time = Some(Instant::now());
                                            slots[frame_slot].needs_zone_check = true;
                                            active_fingers += 1;
                                        }
                                    }
                                    InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_POSITION_X) => {
                                        let val = fev.value() as f32;
                                        slots[frame_slot].current_x = val;
                                        if slots[frame_slot].start_x.is_none() {
                                            slots[frame_slot].start_x = Some(val);
                                        }
                                    }
                                    InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_POSITION_Y) => {
                                        let val = fev.value() as f32;
                                        slots[frame_slot].current_y = val;
                                        if slots[frame_slot].start_y.is_none() {
                                            slots[frame_slot].start_y = Some(val);
                                        }
                                    }
                                    _ => {}
                                }
                            }

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
                                        }
                                        slots[i].needs_zone_check = false;
                                    }
                                }
                            }

                            current_slot = frame_slot;

                            let mut output_events: Vec<InputEvent> = Vec::new();

                            let mut filter_slot: usize = entry_slot;

                            let mut pending_slot_event: Option<InputEvent> = None;

                            for fev in &frame_events {
                                match fev.kind() {
                                    InputEventKind::AbsAxis(AbsoluteAxisType::ABS_MT_SLOT) => {
                                        filter_slot = (fev.value() as usize).min(SLOT_COUNT - 1);
                                        pending_slot_event = Some(*fev);
                                    }
                                    _ if is_single_touch_emulation(fev) => {}
                                    InputEventKind::AbsAxis(axis) if is_mt_axis(axis) => {
                                        if !slots[filter_slot].claimed {
                                            if let Some(slot_ev) = pending_slot_event.take() {
                                                output_events.push(slot_ev);
                                            }
                                            output_events.push(*fev);
                                        }
                                    }
                                    _ => {
                                        output_events.push(*fev);
                                    }
                                }
                            }

                            let unclaimed_count = count_unclaimed(&slots);
                            let primary = find_primary_unclaimed(&slots);

                            let btn_touch_now = unclaimed_count > 0;
                            if btn_touch_now != prev_btn_touch {
                                output_events.push(InputEvent::new(
                                    evdev::EventType::KEY,
                                    BTN_TOUCH,
                                    if btn_touch_now { 1 } else { 0 },
                                ));
                                prev_btn_touch = btn_touch_now;
                            }

                            if unclaimed_count != prev_unclaimed_count {
                                if let Some(old_code) = btn_tool_for_count(prev_unclaimed_count) {
                                    output_events.push(InputEvent::new(
                                        evdev::EventType::KEY, old_code, 0,
                                    ));
                                }
                                if let Some(new_code) = btn_tool_for_count(unclaimed_count) {
                                    output_events.push(InputEvent::new(
                                        evdev::EventType::KEY, new_code, 1,
                                    ));
                                }
                                prev_unclaimed_count = unclaimed_count;
                            }

                            if let Some(p) = primary {
                                output_events.push(InputEvent::new(
                                    evdev::EventType::ABSOLUTE,
                                    AbsoluteAxisType::ABS_X.0,
                                    slots[p].current_x as i32,
                                ));
                                output_events.push(InputEvent::new(
                                    evdev::EventType::ABSOLUTE,
                                    AbsoluteAxisType::ABS_Y.0,
                                    slots[p].current_y as i32,
                                ));
                            }

                            if !output_events.is_empty() {
                                output_events.push(InputEvent::new(
                                    evdev::EventType::SYNCHRONIZATION,
                                    evdev::Synchronization::SYN_REPORT.0,
                                    0,
                                ));
                                if let Err(e) = virtual_device.emit(&output_events) {
                                    error!("Failed to write to virtual device: {}", e);
                                }
                            }

                            frame_events.clear();
                        } else {
                            frame_events.push(ev);
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

/// Returns true if the given absolute axis is a multitouch (ABS_MT_*) axis.
fn is_mt_axis(axis: AbsoluteAxisType) -> bool {
    // MT axes are in the range 0x2f..=0x3f (ABS_MT_SLOT through ABS_MT_TOOL_Y).
    let code = axis.0;
    (0x2f..=0x3f).contains(&code)
}
