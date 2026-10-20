use anyhow::{bail, ensure, Context, Result};
use evdev::{
    raw_stream::RawDevice, AbsoluteAxisType as Axis, EventType, InputEvent, InputEventKind, Key,
    Synchronization,
};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tracing::{error, info};

use crate::config::Config;
use crate::gesture::{Contact, GestureEvent, Recognizer};
use crate::passthrough::create_virtual_device;

pub fn find_trackpad() -> Result<RawDevice> {
    for entry in std::fs::read_dir("/dev/input").context("Failed to read /dev/input")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        if let Ok(device) = RawDevice::open(&path) {
            if let Some(abs_axes) = device.supported_absolute_axes() {
                if abs_axes.contains(Axis::ABS_X)
                    && abs_axes.contains(Axis::ABS_Y)
                    && abs_axes.contains(Axis::ABS_MT_POSITION_X)
                    && abs_axes.contains(Axis::ABS_MT_POSITION_Y)
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

pub fn find_device_by_name(target_name: &str) -> Result<RawDevice> {
    for entry in std::fs::read_dir("/dev/input").context("Failed to read /dev/input")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        if let Ok(device) = RawDevice::open(&path) {
            if let Some(name) = device.name() {
                if name == target_name {
                    info!("Found device by name: {} at {:?}", name, path);
                    return Ok(device);
                }
            }
        }
    }
    bail!(
        "No device found with name: '{}'. Check your spelling or run `sudo libinput list-devices`.",
        target_name
    );
}

pub fn list_trackpads() -> Result<()> {
    let mut paths = std::fs::read_dir("/dev/input")
        .context("Failed to read /dev/input")?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    let mut found = 0;
    let mut unreadable = 0;
    for path in paths {
        if !path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("event"))
        {
            continue;
        }
        let device = match RawDevice::open(&path) {
            Ok(device) => device,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        if device.name() == Some("Bezel Virtual Trackpad")
            || device.properties().contains(evdev::PropType::DIRECT)
            || !device
                .supported_keys()
                .is_some_and(|keys| keys.contains(Key::BTN_TOOL_FINGER))
            || !device.supported_absolute_axes().is_some_and(|axes| {
                [
                    Axis::ABS_X,
                    Axis::ABS_Y,
                    Axis::ABS_MT_POSITION_X,
                    Axis::ABS_MT_POSITION_Y,
                    Axis::ABS_MT_SLOT,
                    Axis::ABS_MT_TRACKING_ID,
                ]
                .iter()
                .all(|axis| axes.contains(*axis))
            })
        {
            continue;
        }
        let axes = device.get_abs_state()?;
        let slots = axes[Axis::ABS_MT_SLOT.0 as usize];
        let x = axes[Axis::ABS_MT_POSITION_X.0 as usize];
        let y = axes[Axis::ABS_MT_POSITION_Y.0 as usize];
        if slots.minimum != 0
            || !(0..256).contains(&slots.maximum)
            || x.maximum <= x.minimum
            || y.maximum <= y.minimum
        {
            continue;
        }
        println!(
            "{}\t{}\t{} slots",
            path.display(),
            device.name().unwrap_or("Unknown"),
            slots.maximum + 1
        );
        found += 1;
    }
    if found == 0 {
        println!("No readable, compatible trackpads found.");
    }
    if unreadable > 0 {
        eprintln!("Skipped {unreadable} unreadable input devices; check /dev/input permissions.");
    }
    ensure!(
        found > 0 || unreadable == 0,
        "Device scan is incomplete because input devices could not be read"
    );
    Ok(())
}

#[derive(Clone, Default)]
struct Slot {
    id: Option<i32>,
    axes: BTreeMap<u16, i32>,
}

struct InputState {
    slots: Vec<Slot>,
    forwarded: Vec<Slot>,
    current: usize,
    bounds: (f32, f32, f32, f32),
    recognizer: Recognizer,
}

fn abs(axis: Axis, value: i32) -> InputEvent {
    InputEvent::new(EventType::ABSOLUTE, axis.0, value)
}
fn key(code: Key, value: i32) -> InputEvent {
    InputEvent::new(EventType::KEY, code.0, value)
}
fn tool(count: usize) -> Option<Key> {
    match count {
        1 => Some(Key::BTN_TOOL_FINGER),
        2 => Some(Key::BTN_TOOL_DOUBLETAP),
        3 => Some(Key::BTN_TOOL_TRIPLETAP),
        4 => Some(Key::BTN_TOOL_QUADTAP),
        5.. => Some(Key::BTN_TOOL_QUINTTAP),
        _ => None,
    }
}
fn mt(axis: Axis) -> bool {
    (0x2f..=0x3f).contains(&axis.0)
}
fn emulated(ev: &InputEvent) -> bool {
    matches!(
        ev.kind(),
        InputEventKind::AbsAxis(
            Axis::ABS_X | Axis::ABS_Y | Axis::ABS_PRESSURE | Axis::ABS_TOOL_WIDTH
        ) | InputEventKind::Key(
            Key::BTN_TOUCH
                | Key::BTN_TOOL_FINGER
                | Key::BTN_TOOL_DOUBLETAP
                | Key::BTN_TOOL_TRIPLETAP
                | Key::BTN_TOOL_QUADTAP
                | Key::BTN_TOOL_QUINTTAP
        )
    )
}

impl InputState {
    fn new(count: usize, bounds: (f32, f32, f32, f32)) -> Self {
        Self {
            slots: vec![Slot::default(); count],
            forwarded: vec![Slot::default(); count],
            current: 0,
            bounds,
            recognizer: Recognizer::default(),
        }
    }
    fn contact(&self, slot: &Slot, active: bool) -> Option<Contact> {
        let (xmin, xrange, ymin, yrange) = self.bounds;
        Some(Contact {
            id: slot.id?,
            x: (*slot.axes.get(&Axis::ABS_MT_POSITION_X.0)? as f32 - xmin) / xrange,
            y: (*slot.axes.get(&Axis::ABS_MT_POSITION_Y.0)? as f32 - ymin) / yrange,
            active,
        })
    }
    fn contacts(&self) -> Vec<Contact> {
        self.slots
            .iter()
            .filter_map(|s| self.contact(s, true))
            .collect()
    }
    fn tick(&mut self, now: u64, config: &Config) -> Vec<GestureEvent> {
        self.recognizer.update(&self.contacts(), now, config)
    }
    fn frame(
        &mut self,
        events: &[InputEvent],
        now: u64,
        config: &Config,
    ) -> (Vec<InputEvent>, Vec<GestureEvent>) {
        let mut output = Vec::new();
        let mut ended = Vec::new();
        for ev in events {
            match ev.kind() {
                InputEventKind::AbsAxis(Axis::ABS_MT_SLOT) => self.current = ev.value() as usize,
                InputEventKind::AbsAxis(axis) if mt(axis) => {
                    if self.current >= self.slots.len() {
                        continue;
                    }
                    if axis == Axis::ABS_MT_TRACKING_ID {
                        if let Some(contact) = self.contact(&self.slots[self.current], false) {
                            ended.push(contact);
                        }
                        self.slots[self.current].id = (ev.value() >= 0).then_some(ev.value());
                    } else {
                        self.slots[self.current].axes.insert(axis.0, ev.value());
                    }
                }
                _ if emulated(ev) => {}
                InputEventKind::Synchronization(_) => {}
                _ => output.push(*ev),
            }
        }
        let mut contacts = self.contacts();
        contacts.extend(ended);
        let gestures = self.recognizer.update(&contacts, now, config);
        output.extend(self.forward());
        (output, gestures)
    }
    fn forward(&mut self) -> Vec<InputEvent> {
        let mut output = Vec::new();
        let old_count = self.forwarded.iter().filter(|s| s.id.is_some()).count();
        for (index, slot) in self.slots.iter().enumerate() {
            let visible = slot
                .id
                .filter(|id| !self.recognizer.claimed(*id) && self.contact(slot, true).is_some());
            let old = &mut self.forwarded[index];
            let mut updates = Vec::new();
            if old.id != visible {
                if old.id.is_some() {
                    updates.push(abs(Axis::ABS_MT_TRACKING_ID, -1));
                }
                if let Some(id) = visible {
                    updates.push(abs(Axis::ABS_MT_TRACKING_ID, id));
                }
            }
            if visible.is_some() {
                for (&axis, &value) in &slot.axes {
                    if old.id != visible || old.axes.get(&axis) != Some(&value) {
                        updates.push(InputEvent::new(EventType::ABSOLUTE, axis, value));
                    }
                }
            }
            if !updates.is_empty() {
                output.push(abs(Axis::ABS_MT_SLOT, index as i32));
                output.extend(updates);
            }
            *old = Slot {
                id: visible,
                axes: slot.axes.clone(),
            };
        }
        let count = self.forwarded.iter().filter(|s| s.id.is_some()).count();
        if (count > 0) != (old_count > 0) {
            output.push(key(Key::BTN_TOUCH, i32::from(count > 0)));
        }
        if tool(count) != tool(old_count) {
            if let Some(code) = tool(old_count) {
                output.push(key(code, 0));
            }
            if let Some(code) = tool(count) {
                output.push(key(code, 1));
            }
        }
        if let Some(primary) = self.forwarded.iter().find(|s| s.id.is_some()) {
            for (from, to) in [
                (Axis::ABS_MT_POSITION_X, Axis::ABS_X),
                (Axis::ABS_MT_POSITION_Y, Axis::ABS_Y),
                (Axis::ABS_MT_PRESSURE, Axis::ABS_PRESSURE),
                (Axis::ABS_MT_TOUCH_MAJOR, Axis::ABS_TOOL_WIDTH),
            ] {
                if let Some(&value) = primary.axes.get(&from.0) {
                    output.push(abs(to, value));
                }
            }
        } else if old_count > 0 {
            output.push(abs(Axis::ABS_PRESSURE, 0));
        }
        output
    }
    fn reset(&mut self) -> Vec<InputEvent> {
        self.recognizer.reset();
        for slot in &mut self.slots {
            slot.id = None;
        }
        self.forward()
    }
}

pub async fn run_input_reader(
    config_rx: tokio::sync::watch::Receiver<Config>,
    gesture_tx: tokio::sync::mpsc::Sender<GestureEvent>,
) -> Result<()> {
    let config = config_rx.borrow().clone();
    let mut device = if config.device.path == "auto" {
        find_trackpad()?
    } else if config.device.path.starts_with('/') {
        RawDevice::open(&config.device.path)
            .with_context(|| format!("Failed to open {}", config.device.path))?
    } else {
        find_device_by_name(&config.device.path)?
    };
    let axes = device.get_abs_state()?;
    let x = axes[Axis::ABS_MT_POSITION_X.0 as usize];
    let y = axes[Axis::ABS_MT_POSITION_Y.0 as usize];
    let slot = axes[Axis::ABS_MT_SLOT.0 as usize];
    ensure!(
        x.maximum > x.minimum && y.maximum > y.minimum,
        "Trackpad has invalid coordinate bounds"
    );
    ensure!(
        device
            .supported_absolute_axes()
            .is_some_and(|a| a.contains(Axis::ABS_MT_SLOT))
            && slot.minimum == 0
            && (0..256).contains(&slot.maximum),
        "Trackpad must support type-B multitouch slots"
    );
    device
        .grab()
        .context("Failed to grab trackpad; check permissions and whether another daemon owns it")?;
    let mut virtual_device = create_virtual_device(&device)?;
    let mut state = InputState::new(
        slot.maximum as usize + 1,
        (
            x.minimum as f32,
            (x.maximum - x.minimum) as f32,
            y.minimum as f32,
            (y.maximum - y.minimum) as f32,
        ),
    );
    state.current = slot.value as usize;
    let mut recovering = device.get_key_state()?.contains(Key::BTN_TOUCH);
    let supported_axes = device
        .supported_absolute_axes()
        .map(|axes| axes.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut stream = device.into_event_stream()?;
    let mut frame = Vec::new();
    let clock = Instant::now();
    let mut timer = tokio::time::interval(Duration::from_millis(5));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    info!(
        "Input reader ready ({} multitouch slots)",
        state.slots.len()
    );
    loop {
        let mut output = Vec::new();
        let gestures;
        tokio::select! {
            event = stream.next_event() => {
                let event = match event {
                    Ok(event) => event,
                    Err(e) => {
                        output.extend(state.reset());
                        output.push(InputEvent::new(EventType::SYNCHRONIZATION, Synchronization::SYN_REPORT.0, 0));
                        let _ = virtual_device.emit(&output);
                        return Err(e).context("Trackpad disconnected");
                    }
                };
                match event.kind() {
                    InputEventKind::Synchronization(Synchronization::SYN_DROPPED) => {
                        error!("Input overflow; cancelling gestures and waiting for fingers to lift");
                        output = state.reset(); frame.clear(); recovering = true; gestures = Vec::new();
                    }
                    InputEventKind::Synchronization(Synchronization::SYN_REPORT) => {
                        if recovering {
                            if !stream.device().get_key_state()?.contains(Key::BTN_TOUCH) {
                                recovering = false;
                                state.current = stream.device().get_abs_state()?[Axis::ABS_MT_SLOT.0 as usize].value as usize;
                                for slot in &mut state.slots { slot.axes.clear(); }
                            }
                            frame.clear(); continue;
                        }
                        (output, gestures) = state.frame(&frame, clock.elapsed().as_millis() as u64, &config_rx.borrow());
                        frame.clear();
                    }
                    _ => { if !recovering { frame.push(event); } continue; }
                }
            }
            _ = timer.tick() => {
                if recovering || !frame.is_empty() { continue; }
                gestures = state.tick(clock.elapsed().as_millis() as u64, &config_rx.borrow());
            }
        }
        output.retain(|ev| match ev.kind() {
            InputEventKind::AbsAxis(axis) => supported_axes.contains(&axis),
            _ => true,
        });
        if !output.is_empty() {
            output.push(InputEvent::new(
                EventType::SYNCHRONIZATION,
                Synchronization::SYN_REPORT.0,
                0,
            ));
            virtual_device
                .emit(&output)
                .context("Failed to forward trackpad input")?;
        }
        for gesture in gestures {
            // Input forwarding must not wait on a slow command consumer.
            if let Err(e) = gesture_tx.try_send(gesture) {
                error!("Gesture queue full or closed: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gesture::Gesture;
    fn start(slot: i32, id: i32, x: i32, y: i32) -> Vec<InputEvent> {
        vec![
            abs(Axis::ABS_MT_SLOT, slot),
            abs(Axis::ABS_MT_TRACKING_ID, id),
            abs(Axis::ABS_MT_POSITION_X, x),
            abs(Axis::ABS_MT_POSITION_Y, y),
        ]
    }
    fn release(slot: i32) -> Vec<InputEvent> {
        vec![
            abs(Axis::ABS_MT_SLOT, slot),
            abs(Axis::ABS_MT_TRACKING_ID, -1),
        ]
    }
    fn state() -> InputState {
        InputState::new(12, (0.0, 1000.0, 0.0, 1000.0))
    }
    fn has(events: &[InputEvent], kind: InputEventKind, value: i32) -> bool {
        events
            .iter()
            .any(|e| e.kind() == kind && e.value() == value)
    }
    #[test]
    fn center_is_forwarded_while_edge_group_is_hidden() {
        let mut s = state();
        let c = Config::default();
        let (output, _) = s.frame(&start(7, 10, 500, 500), 0, &c);
        assert!(has(
            &output,
            InputEventKind::AbsAxis(Axis::ABS_MT_TRACKING_ID),
            10
        ));
        assert!(has(&output, InputEventKind::Key(Key::BTN_TOOL_FINGER), 1));
        let (output, _) = s.frame(&start(0, 11, 10, 500), 20, &c);
        assert!(!has(
            &output,
            InputEventKind::AbsAxis(Axis::ABS_MT_TRACKING_ID),
            11
        ));
        let (output, _) = s.frame(&start(2, 12, 300, 500), 40, &c);
        assert!(!has(
            &output,
            InputEventKind::AbsAxis(Axis::ABS_MT_TRACKING_ID),
            12
        ));
        assert!(!has(
            &output,
            InputEventKind::Key(Key::BTN_TOOL_DOUBLETAP),
            1
        ));
        let (output, events) = s.frame(&release(0), 80, &c);
        assert!(events.is_empty());
        assert!(!has(&output, InputEventKind::Key(Key::BTN_TOUCH), 0));
        s.frame(&release(2), 100, &c);
        let (output, _) = s.frame(&release(7), 120, &c);
        assert!(has(
            &output,
            InputEventKind::AbsAxis(Axis::ABS_MT_TRACKING_ID),
            -1
        ));
        assert!(has(&output, InputEventKind::Key(Key::BTN_TOUCH), 0));
    }
    #[test]
    fn current_slot_is_explicit_even_after_hidden_slot_changes() {
        let mut s = state();
        let c = Config::default();
        s.frame(&start(0, 1, 10, 500), 0, &c);
        s.frame(&start(9, 2, 500, 500), 100, &c);
        s.frame(
            &[abs(Axis::ABS_MT_SLOT, 0), abs(Axis::ABS_MT_POSITION_Y, 400)],
            110,
            &c,
        );
        let (out, _) = s.frame(
            &[abs(Axis::ABS_MT_SLOT, 9), abs(Axis::ABS_MT_POSITION_X, 550)],
            120,
            &c,
        );
        assert!(has(
            &out[..1],
            InputEventKind::AbsAxis(Axis::ABS_MT_SLOT),
            9
        ));
        let (out, _) = s.frame(&[abs(Axis::ABS_MT_POSITION_X, 600)], 130, &c);
        assert!(has(
            &out[..1],
            InputEventKind::AbsAxis(Axis::ABS_MT_SLOT),
            9
        ));
        assert!(has(&out, InputEventKind::AbsAxis(Axis::ABS_X), 600));
    }
    #[test]
    fn released_frame_coordinates_and_slot_reuse() {
        let mut s = state();
        let c = Config::default();
        s.frame(&start(0, 1, 10, 700), 0, &c);
        let mut end = vec![abs(Axis::ABS_MT_POSITION_Y, 400)];
        end.extend(release(0));
        let (_, events) = s.frame(&end, 100, &c);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].gesture, Gesture::SwipeUp);
        let (out, _) = s.frame(&start(0, 2, 500, 500), 150, &c);
        assert!(has(
            &out,
            InputEventKind::AbsAxis(Axis::ABS_MT_TRACKING_ID),
            2
        ));
        let (out, _) = s.frame(&start(0, 3, 550, 550), 160, &c);
        assert!(has(
            &out,
            InputEventKind::AbsAxis(Axis::ABS_MT_TRACKING_ID),
            -1
        ));
        assert!(has(
            &out,
            InputEventKind::AbsAxis(Axis::ABS_MT_TRACKING_ID),
            3
        ));
    }
    #[test]
    fn replacing_edge_contact_in_one_frame_starts_a_new_gesture() {
        let mut s = state();
        let c = Config::default();
        s.frame(&start(0, 1, 10, 500), 0, &c);
        let (out, first) = s.frame(&start(0, 2, 10, 500), 100, &c);
        assert!(out.is_empty());
        assert_eq!(first.len(), 1);
        let (out, second) = s.frame(&release(0), 150, &c);
        assert!(out.is_empty());
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].gesture, Gesture::Tap);
    }

    #[test]
    fn reset_releases_all_forwarded_contacts_and_cancels_gestures() {
        let mut s = state();
        let c = Config::default();
        s.frame(&start(0, 1, 500, 500), 0, &c);
        s.frame(&start(1, 2, 600, 500), 10, &c);
        s.frame(&start(2, 3, 10, 500), 20, &c);
        let out = s.reset();
        assert_eq!(
            out.iter()
                .filter(
                    |e| e.kind() == InputEventKind::AbsAxis(Axis::ABS_MT_TRACKING_ID)
                        && e.value() == -1
                )
                .count(),
            2
        );
        assert!(has(&out, InputEventKind::Key(Key::BTN_TOOL_DOUBLETAP), 0));
        assert!(has(&out, InputEventKind::Key(Key::BTN_TOUCH), 0));
        assert!(s.tick(600, &c).is_empty());
    }
    #[test]
    fn incomplete_coordinates_and_invalid_slots_are_not_forwarded() {
        let mut s = state();
        let c = Config::default();
        let (out, _) = s.frame(
            &[
                abs(Axis::ABS_MT_TRACKING_ID, 1),
                abs(Axis::ABS_MT_POSITION_X, 10),
            ],
            0,
            &c,
        );
        assert!(out.is_empty());
        let (out, _) = s.frame(&[abs(Axis::ABS_MT_POSITION_Y, 500)], 10, &c);
        assert!(out.is_empty());
        s.frame(&start(100, 2, 500, 500), 20, &c);
        assert_eq!(s.contacts().len(), 1);
    }
}
