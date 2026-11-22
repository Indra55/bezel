use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::watch;

use crate::config::{Config, GestureAction, Recognition};
use crate::focus::{same_focus, Focus, FocusedWindow};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Zone {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    Tap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gesture {
    Tap,
    DoubleTap,
    Hold,
    SwipeUp,
    SwipeDown,
    SwipeLeft,
    SwipeRight,
    SwipeUpLeft,
    SwipeUpRight,
    SwipeDownLeft,
    SwipeDownRight,
    SlideUp,
    SlideDown,
    SlideLeft,
    SlideRight,
}

impl From<Direction> for Gesture {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Up => Self::SwipeUp,
            Direction::Down => Self::SwipeDown,
            Direction::Left => Self::SwipeLeft,
            Direction::Right => Self::SwipeRight,
            Direction::Tap => Self::Tap,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GestureEvent {
    pub zone: Zone,
    pub gesture: Gesture,
    pub magnitude: f32,
    pub finger_count: u8,
    pub action: GestureAction,
    pub target: Focus,
}

pub struct ActionCommand {
    pub cmd: String,
    pub osd_message: Option<String>,
    pub target: Focus,
}

#[derive(Debug, Clone, Copy)]
pub struct Contact {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub active: bool,
}

struct Member {
    start: Contact,
    dx: f32,
    dy: f32,
    active: bool,
}

struct Session {
    config: Config,
    focus: Focus,
    zone: Zone,
    started: u64,
    members: Vec<Member>,
    frozen: bool,
    consumed: bool,
    sliding: bool,
    cancelled: bool,
    max_motion: f32,
    slide_anchor: (f32, f32),
    last_motion: (f32, f32),
}

struct PendingTap {
    zone: Zone,
    fingers: u8,
    start: Contact,
    released: u64,
    deadline: u64,
    tolerance: f32,
    event: Option<GestureEvent>,
    target: Focus,
}

#[derive(Default)]
pub struct Recognizer {
    session: Option<Session>,
    previous: HashSet<i32>,
    pending: Option<PendingTap>,
    focus: Option<watch::Receiver<Focus>>,
}

pub fn determine_zone(x: f32, y: f32, config: &Config) -> Option<Zone> {
    if x < config.zones.left_width {
        Some(Zone::Left)
    } else if x > 1.0 - config.zones.right_width {
        Some(Zone::Right)
    } else if y < config.zones.top_height {
        Some(Zone::Top)
    } else if y > 1.0 - config.zones.bottom_height {
        Some(Zone::Bottom)
    } else {
        None
    }
}

struct GestureBinding<'a> {
    action: GestureAction,
    settings: Recognition,
    target: Option<&'a Arc<FocusedWindow>>,
}

impl Session {
    fn fingers(&self) -> u8 {
        self.members.len() as u8
    }
    fn binding(&self, gesture: Gesture) -> Option<GestureBinding<'_>> {
        if let Some(window) = &self.focus {
            if let Some((action, settings)) =
                self.config
                    .app_binding(&window.app_id, self.zone, self.fingers(), gesture)
            {
                return Some(GestureBinding {
                    action,
                    settings,
                    target: Some(window),
                });
            }
        }
        let (action, settings) = self.config.binding(self.zone, self.fingers(), gesture)?;
        Some(GestureBinding {
            action,
            settings,
            target: None,
        })
    }
    fn event(&self, gesture: Gesture, magnitude: f32) -> Option<GestureEvent> {
        self.binding(gesture).map(|binding| GestureEvent {
            zone: self.zone,
            gesture,
            magnitude,
            finger_count: self.fingers(),
            action: binding.action,
            target: binding.target.cloned(),
        })
    }
    fn motion(&self) -> (f32, f32) {
        let n = self.members.len() as f32;
        (
            self.members.iter().map(|m| m.dx).sum::<f32>() / n,
            self.members.iter().map(|m| m.dy).sum::<f32>() / n,
        )
    }
    fn emit(&self, gesture: Gesture, magnitude: f32, out: &mut Vec<GestureEvent>) {
        if let Some(event) = self.event(gesture, magnitude) {
            out.push(event);
        }
    }
}

impl Recognizer {
    pub fn with_focus(focus: watch::Receiver<Focus>) -> Self {
        Self {
            focus: Some(focus),
            ..Self::default()
        }
    }

    pub fn claimed(&self, id: i32) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| s.members.iter().any(|m| m.start.id == id))
    }
    pub fn reset(&mut self) {
        self.session = None;
        self.previous.clear();
        self.pending = None;
    }

    /// Advance using complete input frames, or the last frame when a timer expires.
    pub fn update(&mut self, contacts: &[Contact], now: u64, config: &Config) -> Vec<GestureEvent> {
        let mut out = Vec::new();
        let fresh: Vec<_> = contacts
            .iter()
            .filter(|c| c.active && !self.previous.contains(&c.id))
            .copied()
            .collect();
        self.previous = contacts.iter().filter(|c| c.active).map(|c| c.id).collect();
        if self.session.is_none() {
            if let Some((first, zone)) = fresh
                .iter()
                .find_map(|c| determine_zone(c.x, c.y, config).map(|z| (*c, z)))
            {
                self.session = Some(Session {
                    config: config.clone(),
                    focus: self.focus.as_ref().and_then(|focus| focus.borrow().clone()),
                    zone,
                    started: now,
                    members: vec![Member {
                        start: first,
                        dx: 0.0,
                        dy: 0.0,
                        active: true,
                    }],
                    frozen: false,
                    consumed: false,
                    sliding: false,
                    cancelled: false,
                    max_motion: 0.0,
                    slide_anchor: (0.0, 0.0),
                    last_motion: (0.0, 0.0),
                });
            }
        }
        if let Some(s) = self.session.as_mut() {
            let elapsed = now.saturating_sub(s.started);
            if elapsed >= s.config.recognition.join_ms
                || s.members
                    .iter()
                    .any(|m| !contacts.iter().any(|c| c.active && c.id == m.start.id))
            {
                s.frozen = true;
            }
            if !s.frozen {
                for c in &fresh {
                    if !s.members.iter().any(|m| m.start.id == c.id) {
                        s.members.push(Member {
                            start: *c,
                            dx: 0.0,
                            dy: 0.0,
                            active: true,
                        });
                    }
                }
                if s.members.len() > 4 {
                    s.cancelled = true;
                }
            }
            for m in &mut s.members {
                m.active = contacts.iter().any(|c| c.active && c.id == m.start.id);
                if let Some(c) = contacts.iter().find(|c| c.id == m.start.id) {
                    m.dx = c.x - m.start.x;
                    m.dy = c.y - m.start.y;
                    s.max_motion = s.max_motion.max(m.dx.hypot(m.dy));
                }
            }
            let motion = s.motion();
            let ended = s.members.iter().all(|m| !m.active);
            let all_down = s.members.iter().all(|m| m.active);
            if s.frozen && !s.cancelled && !s.consumed && all_down {
                if let Some(GestureBinding { settings, .. }) = s.binding(Gesture::Hold) {
                    if elapsed >= settings.hold_ms && s.max_motion <= settings.tap_distance {
                        s.emit(Gesture::Hold, 0.0, &mut out);
                        s.consumed = true;
                    }
                }
            }
            if s.frozen && !s.cancelled && (!s.consumed || s.sliding) && all_down {
                // Discard the unused fraction at a direction reversal for immediate feedback.
                if (motion.0 - s.last_motion.0) * (s.last_motion.0 - s.slide_anchor.0) < 0.0 {
                    s.slide_anchor.0 = s.last_motion.0;
                }
                if (motion.1 - s.last_motion.1) * (s.last_motion.1 - s.slide_anchor.1) < 0.0 {
                    s.slide_anchor.1 = s.last_motion.1;
                }
                let dx = motion.0 - s.slide_anchor.0;
                let dy = motion.1 - s.slide_anchor.1;
                let (gesture, distance, horizontal) = if dx.abs() > dy.abs() {
                    (
                        if dx > 0.0 {
                            Gesture::SlideRight
                        } else {
                            Gesture::SlideLeft
                        },
                        dx.abs(),
                        true,
                    )
                } else {
                    (
                        if dy > 0.0 {
                            Gesture::SlideDown
                        } else {
                            Gesture::SlideUp
                        },
                        dy.abs(),
                        false,
                    )
                };
                if let Some(GestureBinding { settings, .. }) = s.binding(gesture) {
                    let steps = (distance / settings.step_distance).floor() as usize;
                    for _ in 0..steps {
                        s.emit(gesture, settings.step_distance, &mut out);
                    }
                    if steps > 0 {
                        s.sliding = true;
                        s.consumed = true;
                        if horizontal {
                            s.slide_anchor.0 += dx.signum() * steps as f32 * settings.step_distance;
                            s.slide_anchor.1 = motion.1;
                        } else {
                            s.slide_anchor.1 += dy.signum() * steps as f32 * settings.step_distance;
                            s.slide_anchor.0 = motion.0;
                        }
                    }
                }
            }
            s.last_motion = motion;
            if ended {
                let s = self.session.take().unwrap();
                if !s.consumed && !s.cancelled {
                    let (dx, dy) = motion;
                    let magnitude = dx.hypot(dy);
                    let diagonal = match (dx >= 0.0, dy >= 0.0) {
                        (true, true) => Gesture::SwipeDownRight,
                        (true, false) => Gesture::SwipeUpRight,
                        (false, true) => Gesture::SwipeDownLeft,
                        (false, false) => Gesture::SwipeUpLeft,
                    };
                    let direction = if dx.abs().min(dy.abs())
                        >= dx.abs().max(dy.abs()) * (std::f32::consts::PI / 8.0).tan()
                        && s.binding(diagonal).is_some()
                    {
                        diagonal
                    } else if dx.abs() > dy.abs() {
                        if dx > 0.0 {
                            Gesture::SwipeRight
                        } else {
                            Gesture::SwipeLeft
                        }
                    } else if dy > 0.0 {
                        Gesture::SwipeDown
                    } else {
                        Gesture::SwipeUp
                    };
                    if s.binding(direction)
                        .is_some_and(|b| magnitude >= b.settings.swipe_distance)
                    {
                        s.emit(direction, magnitude, &mut out);
                    } else {
                        let double = s.binding(Gesture::DoubleTap);
                        let single = s.binding(Gesture::Tap);
                        let tap_target = double
                            .as_ref()
                            .and_then(|b| b.target)
                            .or_else(|| single.as_ref().and_then(|b| b.target))
                            .cloned();
                        let qualifies =
                            |r: &Recognition| elapsed <= r.tap_ms && s.max_motion <= r.tap_distance;
                        let start = s.members[0].start;
                        let matches_pending = self.pending.as_ref().is_some_and(|p| {
                            p.zone == s.zone
                                && p.fingers == s.fingers()
                                && s.started >= p.released
                                && now <= p.deadline
                                && same_focus(&p.target, &tap_target)
                                && (start.x - p.start.x).hypot(start.y - p.start.y) <= p.tolerance
                        });
                        if matches_pending
                            && double.as_ref().is_some_and(|b| qualifies(&b.settings))
                        {
                            self.pending = None;
                            s.emit(Gesture::DoubleTap, magnitude, &mut out);
                        } else {
                            if let Some(p) = self.pending.take() {
                                if let Some(event) = p.event {
                                    out.push(event);
                                }
                            }
                            let event = single
                                .filter(|b| qualifies(&b.settings))
                                .and_then(|_| s.event(Gesture::Tap, magnitude));
                            if let Some(GestureBinding { settings, .. }) =
                                double.filter(|b| qualifies(&b.settings))
                            {
                                self.pending = Some(PendingTap {
                                    zone: s.zone,
                                    fingers: s.fingers(),
                                    start,
                                    released: now,
                                    deadline: now + settings.double_tap_ms,
                                    tolerance: settings.tap_distance,
                                    event,
                                    target: tap_target,
                                });
                            } else if let Some(event) = event {
                                out.push(event);
                            }
                        }
                    }
                }
            }
        }
        // A slot can end one contact and begin another in the same input frame.
        if self.session.is_none()
            && fresh
                .iter()
                .any(|c| determine_zone(c.x, c.y, config).is_some())
        {
            for contact in &fresh {
                self.previous.remove(&contact.id);
            }
            out.extend(self.update(contacts, now, config));
        }
        if self.pending.as_ref().is_some_and(|p| now >= p.deadline) {
            if let Some(event) = self.pending.take().and_then(|p| p.event) {
                out.push(event);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(gestures: &[(&str, u8)]) -> Config {
        let text = gestures.iter().map(|(g, n)| format!("[[bindings]]\nzone='left'\nfingers={n}\ngesture='{g}'\naction='command'\ncmd='{g}'\n")).collect::<String>();
        let c: Config = toml::from_str(&text).unwrap();
        c.validate().unwrap();
        c
    }
    fn finger(id: i32, x: f32, y: f32) -> Contact {
        Contact {
            id,
            x,
            y,
            active: true,
        }
    }
    fn kinds(events: Vec<GestureEvent>) -> Vec<Gesture> {
        events.into_iter().map(|e| e.gesture).collect()
    }

    #[test]
    fn grouped_swipe_has_fixed_count_and_one_action() {
        for count in 1..=4 {
            let c = config(&[("swipe_up", count)]);
            let mut r = Recognizer::default();
            let mut fingers = vec![finger(0, 0.01, 0.7)];
            r.update(&fingers, 0, &c);
            for id in 1..count {
                fingers.push(finger(id as i32, 0.2 * id as f32, 0.7));
            }
            r.update(&fingers, 40, &c);
            for f in &mut fingers {
                f.y -= 0.2;
            }
            assert!(r.update(&fingers, 100, &c).is_empty());
            while fingers.len() > 1 {
                fingers.pop();
                assert!(r.update(&fingers, 150, &c).is_empty());
            }
            let events = r.update(&[], 180, &c);
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].finger_count, count);
            assert_eq!(events[0].gesture, Gesture::SwipeUp);
            assert!(r.update(&[], 200, &c).is_empty());
        }
    }
    #[test]
    fn center_contacts_and_late_arrivals_are_not_claimed() {
        let c = config(&[("tap", 1)]);
        let mut r = Recognizer::default();
        let center = finger(1, 0.5, 0.5);
        let edge = finger(2, 0.01, 0.5);
        r.update(&[center], 0, &c);
        r.update(&[center, edge], 10, &c);
        assert!(!r.claimed(1));
        assert!(r.claimed(2));
        r.update(&[center, edge, finger(3, 0.3, 0.5)], 100, &c);
        assert!(!r.claimed(3));
        assert_eq!(kinds(r.update(&[center], 150, &c)), vec![Gesture::Tap]);
    }
    #[test]
    fn holds_fire_without_motion_and_suppress_release() {
        let c = config(&[("hold", 2), ("tap", 2), ("swipe_up", 2)]);
        let mut r = Recognizer::default();
        let fingers = [finger(1, 0.01, 0.5), finger(2, 0.3, 0.5)];
        r.update(&fingers, 0, &c);
        assert!(r.update(&fingers, 499, &c).is_empty());
        assert_eq!(kinds(r.update(&fingers, 500, &c)), vec![Gesture::Hold]);
        assert!(r.update(&fingers, 700, &c).is_empty());
        assert!(r.update(&[], 800, &c).is_empty());
    }
    #[test]
    fn double_tap_delays_single_only_when_bound() {
        let c = config(&[("tap", 1), ("double_tap", 1)]);
        let mut r = Recognizer::default();
        r.update(&[finger(1, 0.01, 0.5)], 0, &c);
        assert!(r.update(&[], 50, &c).is_empty());
        r.update(&[finger(2, 0.01, 0.5)], 100, &c);
        assert_eq!(kinds(r.update(&[], 150, &c)), vec![Gesture::DoubleTap]);
        assert!(r.update(&[], 500, &c).is_empty());
        r.update(&[finger(3, 0.01, 0.5)], 600, &c);
        r.update(&[], 650, &c);
        assert!(r.update(&[], 949, &c).is_empty());
        assert_eq!(kinds(r.update(&[], 950, &c)), vec![Gesture::Tap]);
        let c = config(&[("tap", 1)]);
        r.update(&[finger(4, 0.01, 0.5)], 1000, &c);
        assert_eq!(kinds(r.update(&[], 1050, &c)), vec![Gesture::Tap]);
    }
    #[test]
    fn double_tap_requires_same_position_and_count() {
        for second in [
            vec![finger(2, 0.01, 0.8)],
            vec![finger(2, 0.01, 0.5), finger(3, 0.3, 0.5)],
        ] {
            let c = config(&[("tap", 1), ("double_tap", 1), ("tap", 2), ("double_tap", 2)]);
            let mut r = Recognizer::default();
            r.update(&[finger(1, 0.01, 0.5)], 0, &c);
            r.update(&[], 50, &c);
            r.update(&second, 100, &c);
            assert_eq!(kinds(r.update(&[], 150, &c)), vec![Gesture::Tap]);
            assert_eq!(kinds(r.update(&[], 450, &c)), vec![Gesture::Tap]);
        }
    }
    #[test]
    fn slides_repeat_reverse_and_suppress_swipes() {
        let c = config(&[("slide_up", 1), ("slide_down", 1), ("swipe_up", 1)]);
        let mut r = Recognizer::default();
        r.update(&[finger(1, 0.01, 0.7)], 0, &c);
        assert_eq!(
            kinds(r.update(&[finger(1, 0.01, 0.6)], 100, &c)),
            vec![Gesture::SlideUp; 3]
        );
        assert!(r.update(&[finger(1, 0.01, 0.6)], 110, &c).is_empty());
        assert_eq!(
            kinds(r.update(&[finger(1, 0.01, 0.64)], 120, &c)),
            vec![Gesture::SlideDown]
        );
        assert!(r.update(&[], 150, &c).is_empty());
    }
    #[test]
    fn cardinal_and_diagonal_swipes() {
        for (name, gesture, dx, dy) in [
            ("swipe_up", Gesture::SwipeUp, 0.0, -0.2),
            ("swipe_down", Gesture::SwipeDown, 0.0, 0.2),
            ("swipe_left", Gesture::SwipeLeft, -0.2, 0.0),
            ("swipe_right", Gesture::SwipeRight, 0.2, 0.0),
            ("swipe_up_left", Gesture::SwipeUpLeft, -0.2, -0.2),
            ("swipe_up_right", Gesture::SwipeUpRight, 0.2, -0.2),
            ("swipe_down_left", Gesture::SwipeDownLeft, -0.2, 0.2),
            ("swipe_down_right", Gesture::SwipeDownRight, 0.2, 0.2),
        ] {
            let c = config(&[(name, 1)]);
            let mut r = Recognizer::default();
            r.update(&[finger(1, 0.01, 0.5)], 0, &c);
            r.update(&[finger(1, 0.01 + dx, 0.5 + dy)], 100, &c);
            assert_eq!(kinds(r.update(&[], 150, &c)), vec![gesture]);
        }
        let c = config(&[("swipe_down", 1)]);
        let mut r = Recognizer::default();
        r.update(&[finger(1, 0.01, 0.5)], 0, &c);
        r.update(&[finger(1, 0.21, 0.8)], 100, &c);
        assert_eq!(kinds(r.update(&[], 150, &c)), vec![Gesture::SwipeDown]);
    }
    #[test]
    fn no_tap_after_excursion_or_group_overflow() {
        let c = config(&[("tap", 1)]);
        let mut r = Recognizer::default();
        r.update(&[finger(1, 0.01, 0.5)], 0, &c);
        r.update(&[finger(1, 0.3, 0.5)], 80, &c);
        r.update(&[finger(1, 0.01, 0.5)], 100, &c);
        assert!(r.update(&[], 150, &c).is_empty());
        let fingers: Vec<_> = (0..5).map(|id| finger(id, 0.01, 0.5)).collect();
        r.update(&fingers, 200, &c);
        assert!(r.update(&[], 250, &c).is_empty());
    }
    #[test]
    fn reload_and_reset_do_not_change_inflight_actions() {
        let c = config(&[("tap", 1), ("double_tap", 1)]);
        let empty = config(&[]);
        let mut r = Recognizer::default();
        r.update(&[finger(1, 0.01, 0.5)], 0, &c);
        r.update(&[], 50, &empty);
        assert_eq!(kinds(r.update(&[], 350, &empty)), vec![Gesture::Tap]);
        r.update(&[finger(2, 0.01, 0.5)], 400, &c);
        r.update(&[], 450, &c);
        r.reset();
        assert!(r.update(&[], 800, &c).is_empty());
    }

    #[test]
    fn app_swipe_keeps_the_window_and_binding_from_touch_down() {
        let config: Config = toml::from_str(
            r#"
            [gestures.bottom.right]
            action = "command"
            cmd = "global"

            [[apps.firefox.bindings]]
            zone = "bottom"
            fingers = 1
            gesture = "swipe_right"
            action = "command"
            cmd = "next tab"

            [[apps.vlc.bindings]]
            zone = "bottom"
            fingers = 1
            gesture = "swipe_right"
            action = "command"
            cmd = "seek forward"
        "#,
        )
        .unwrap();
        config.validate().unwrap();
        let browser = Arc::new(FocusedWindow {
            app_id: "firefox".into(),
            id: "12".into(),
        });
        let player = Arc::new(FocusedWindow {
            app_id: "vlc".into(),
            id: "14".into(),
        });
        let (tx, rx) = watch::channel(Some(browser.clone()));
        let mut recognizer = Recognizer::with_focus(rx);

        recognizer.update(&[finger(1, 0.4, 0.98)], 0, &config);
        tx.send_replace(Some(player.clone()));
        let released = |id| Contact {
            active: false,
            ..finger(id, 0.7, 0.98)
        };
        let first = recognizer.update(&[released(1)], 100, &config);
        assert_eq!(first[0].action.cmd, "next tab");
        assert!(Arc::ptr_eq(first[0].target.as_ref().unwrap(), &browser));

        // A reset after input overflow keeps observing focus for future gestures.
        recognizer.reset();
        recognizer.update(&[finger(2, 0.4, 0.98)], 200, &config);
        let second = recognizer.update(&[released(2)], 300, &config);
        assert_eq!(second[0].action.cmd, "seek forward");
        assert!(Arc::ptr_eq(second[0].target.as_ref().unwrap(), &player));

        tx.send_replace(None);
        recognizer.update(&[finger(3, 0.4, 0.98)], 400, &config);
        let third = recognizer.update(&[released(3)], 500, &config);
        assert_eq!(third[0].action.cmd, "global");
        assert!(third[0].target.is_none());
    }

    #[test]
    fn app_double_tap_does_not_pair_across_windows_of_the_same_app() {
        let config: Config = toml::from_str(
            r#"
            [[apps.firefox.bindings]]
            zone = "left"
            fingers = 1
            gesture = "double_tap"
            action = "command"
            cmd = "double tap"
        "#,
        )
        .unwrap();
        let focus = |id: &str| {
            Some(Arc::new(FocusedWindow {
                app_id: "firefox".into(),
                id: id.into(),
            }))
        };
        let (tx, rx) = watch::channel(focus("1"));
        let mut recognizer = Recognizer::with_focus(rx);
        let released = |id| Contact {
            active: false,
            ..finger(id, 0.01, 0.5)
        };
        recognizer.update(&[finger(1, 0.01, 0.5)], 0, &config);
        assert!(recognizer.update(&[released(1)], 30, &config).is_empty());
        tx.send_replace(focus("2"));
        recognizer.update(&[finger(2, 0.01, 0.5)], 60, &config);
        assert!(recognizer.update(&[released(2)], 90, &config).is_empty());
        recognizer.update(&[finger(3, 0.01, 0.5)], 120, &config);
        assert_eq!(
            kinds(recognizer.update(&[released(3)], 150, &config)),
            [Gesture::DoubleTap]
        );
    }
}
