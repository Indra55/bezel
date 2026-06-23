use serde::Deserialize;

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

#[derive(Debug, Clone)]
pub struct GestureEvent {
    pub zone: Zone,
    pub direction: Direction,
    pub magnitude: f32, // normalised 0.0–1.0 distance travelled
    pub finger_count: u8,
}

pub struct ActionCommand {
    pub cmd: String,
    pub osd_message: Option<String>,
}

// Basic threshold settings
pub const SWIPE_THRESHOLD: f32 = 0.05;
pub const TAP_THRESHOLD: f32 = 0.02;
pub const MAX_TAP_DURATION_MS: u128 = 200;

pub fn classify_gesture(
    zone: Zone,
    dx: f32,
    dy: f32,
    finger_count: u8,
    duration_ms: u128,
) -> Option<GestureEvent> {
    let abs_dx = dx.abs();
    let abs_dy = dy.abs();
    let magnitude = (dx * dx + dy * dy).sqrt();

    if magnitude < TAP_THRESHOLD && duration_ms < MAX_TAP_DURATION_MS {
        return Some(GestureEvent {
            zone,
            direction: Direction::Tap,
            magnitude,
            finger_count,
        });
    }

    if magnitude >= SWIPE_THRESHOLD {
        let direction = if abs_dx > abs_dy {
            // Dominant axis is X
            if dx > 0.0 {
                Direction::Right
            } else {
                Direction::Left
            }
        } else {
            // Dominant axis is Y
            if dy > 0.0 {
                Direction::Down
            } else {
                Direction::Up
            }
        };

        return Some(GestureEvent {
            zone,
            direction,
            magnitude,
            finger_count,
        });
    }

    None
}
