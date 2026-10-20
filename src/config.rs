use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::gesture::{Direction, Gesture, Zone};

#[derive(Debug, Deserialize, Clone)]
pub struct DeviceConfig {
    #[serde(default = "default_device_path")]
    pub path: String,
}

fn default_device_path() -> String {
    "auto".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct ZonesConfig {
    #[serde(default = "default_zone_width")]
    pub left_width: f32,
    #[serde(default = "default_zone_width")]
    pub right_width: f32,
    #[serde(default = "default_zone_height")]
    pub top_height: f32,
    #[serde(default = "default_zone_height")]
    pub bottom_height: f32,
}

fn default_zone_width() -> f32 {
    0.08
}

fn default_zone_height() -> f32 {
    0.08
}

#[derive(Debug, Deserialize, Clone)]
pub struct GestureAction {
    pub action: String,
    pub cmd: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct Recognition {
    pub join_ms: u64,
    pub swipe_distance: f32,
    pub tap_distance: f32,
    pub tap_ms: u64,
    pub double_tap_ms: u64,
    pub hold_ms: u64,
    pub step_distance: f32,
}

impl Default for Recognition {
    fn default() -> Self {
        Self {
            join_ms: 80,
            swipe_distance: 0.05,
            tap_distance: 0.02,
            tap_ms: 200,
            double_tap_ms: 300,
            hold_ms: 500,
            step_distance: 0.03,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub zone: Zone,
    pub fingers: u8,
    pub gesture: Gesture,
    pub action: String,
    pub cmd: String,
    pub swipe_distance: Option<f32>,
    pub tap_distance: Option<f32>,
    pub tap_ms: Option<u64>,
    pub double_tap_ms: Option<u64>,
    pub hold_ms: Option<u64>,
    pub step_distance: Option<f32>,
}

impl Binding {
    fn settings(&self, defaults: &Recognition) -> Recognition {
        Recognition {
            join_ms: defaults.join_ms,
            swipe_distance: self.swipe_distance.unwrap_or(defaults.swipe_distance),
            tap_distance: self.tap_distance.unwrap_or(defaults.tap_distance),
            tap_ms: self.tap_ms.unwrap_or(defaults.tap_ms),
            double_tap_ms: self.double_tap_ms.unwrap_or(defaults.double_tap_ms),
            hold_ms: self.hold_ms.unwrap_or(defaults.hold_ms),
            step_distance: self.step_distance.unwrap_or(defaults.step_distance),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct OsdConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_osd_backend")]
    pub backend: String,
    #[serde(default)]
    pub canonical_hints: Option<bool>,
    #[serde(default)]
    pub pipe_path: Option<String>,
}

fn default_osd_backend() -> String {
    "notify-send".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub recognition: Recognition,
    #[serde(default)]
    pub bindings: Vec<Binding>,
    #[serde(default)]
    pub device: DeviceConfig,
    #[serde(default)]
    pub zones: ZonesConfig,
    #[serde(default)]
    pub gestures: HashMap<Zone, HashMap<Direction, GestureAction>>,
    #[serde(default)]
    pub osd: OsdConfig,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        DeviceConfig {
            path: default_device_path(),
        }
    }
}

impl Default for ZonesConfig {
    fn default() -> Self {
        ZonesConfig {
            left_width: default_zone_width(),
            right_width: default_zone_width(),
            top_height: default_zone_height(),
            bottom_height: default_zone_height(),
        }
    }
}

impl Default for OsdConfig {
    fn default() -> Self {
        OsdConfig {
            enabled: false,
            backend: default_osd_backend(),
            canonical_hints: None,
            pipe_path: None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut gestures = HashMap::new();

        let mut left_gestures = HashMap::new();
        left_gestures.insert(
            Direction::Up,
            GestureAction {
                action: "command".into(),
                cmd: "wpctl set-volume @DEFAULT_SINK@ 5%+".into(),
            },
        );
        left_gestures.insert(
            Direction::Down,
            GestureAction {
                action: "command".into(),
                cmd: "wpctl set-volume @DEFAULT_SINK@ 5%-".into(),
            },
        );
        left_gestures.insert(
            Direction::Tap,
            GestureAction {
                action: "command".into(),
                cmd: "wpctl set-mute @DEFAULT_SINK@ toggle".into(),
            },
        );
        gestures.insert(Zone::Left, left_gestures);

        let mut right_gestures = HashMap::new();
        right_gestures.insert(
            Direction::Up,
            GestureAction {
                action: "command".into(),
                cmd: "brightnessctl set 10%+".into(),
            },
        );
        right_gestures.insert(
            Direction::Down,
            GestureAction {
                action: "command".into(),
                cmd: "brightnessctl set 10%-".into(),
            },
        );
        gestures.insert(Zone::Right, right_gestures);

        let mut top_gestures = HashMap::new();
        top_gestures.insert(
            Direction::Left,
            GestureAction {
                action: "command".into(),
                cmd: "hyprctl dispatch 'hl.dsp.focus({ workspace = \"e-1\" })'".into(),
            },
        );
        top_gestures.insert(
            Direction::Right,
            GestureAction {
                action: "command".into(),
                cmd: "hyprctl dispatch 'hl.dsp.focus({ workspace = \"e+1\" })'".into(),
            },
        );
        top_gestures.insert(
            Direction::Tap,
            GestureAction {
                action: "command".into(),
                cmd: "hyprctl dispatch 'hl.dsp.workspace.toggle_special(\"magic\")'".into(),
            },
        );
        gestures.insert(Zone::Top, top_gestures);

        let mut bottom_gestures = HashMap::new();
        bottom_gestures.insert(
            Direction::Left,
            GestureAction {
                action: "command".into(),
                cmd: "playerctl previous".into(),
            },
        );
        bottom_gestures.insert(
            Direction::Right,
            GestureAction {
                action: "command".into(),
                cmd: "playerctl next".into(),
            },
        );
        bottom_gestures.insert(
            Direction::Tap,
            GestureAction {
                action: "command".into(),
                cmd: "playerctl play-pause".into(),
            },
        );
        gestures.insert(Zone::Bottom, bottom_gestures);

        Config {
            recognition: Recognition::default(),
            bindings: Vec::new(),
            device: DeviceConfig::default(),
            zones: ZonesConfig::default(),
            gestures,
            osd: OsdConfig::default(),
        }
    }
}

pub fn get_config_path() -> PathBuf {
    if let Some(mut path) = dirs::config_dir() {
        path.push("bezel");
        path.push("config.toml");
        path
    } else {
        PathBuf::from("config.toml")
    }
}

pub fn load_config(path: &Path) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file at {:?}", path))?;
    let config: Config = toml::from_str(&content).context("Failed to parse config.toml")?;

    config.validate()?;
    Ok(config)
}

impl Config {
    pub fn binding(
        &self,
        zone: Zone,
        fingers: u8,
        gesture: Gesture,
    ) -> Option<(GestureAction, Recognition)> {
        if let Some(b) = self
            .bindings
            .iter()
            .find(|b| b.zone == zone && b.fingers == fingers && b.gesture == gesture)
        {
            return Some((
                GestureAction {
                    action: b.action.clone(),
                    cmd: b.cmd.clone(),
                },
                b.settings(&self.recognition),
            ));
        }
        if fingers != 1 {
            return None;
        }
        self.gestures
            .get(&zone)?
            .iter()
            .find(|(d, _)| Gesture::from(**d) == gesture)
            .map(|(_, action)| (action.clone(), self.recognition.clone()))
    }

    pub fn validate(&self) -> Result<()> {
        use anyhow::ensure;
        fn settings(r: &Recognition) -> Result<()> {
            for (name, value) in [
                ("swipe_distance", r.swipe_distance),
                ("tap_distance", r.tap_distance),
                ("step_distance", r.step_distance),
            ] {
                ensure!(
                    value.is_finite() && (0.001..=1.0).contains(&value),
                    "{name} must be between 0.001 and 1"
                );
            }
            ensure!(
                (1..=1000).contains(&r.join_ms),
                "join_ms must be between 1 and 1000"
            );
            for (name, value) in [
                ("tap_ms", r.tap_ms),
                ("double_tap_ms", r.double_tap_ms),
                ("hold_ms", r.hold_ms),
            ] {
                ensure!(
                    (1..=10000).contains(&value),
                    "{name} must be between 1 and 10000"
                );
            }
            ensure!(
                r.join_ms <= r.tap_ms && r.join_ms <= r.hold_ms,
                "join_ms must not exceed tap_ms or hold_ms"
            );
            Ok(())
        }
        settings(&self.recognition).context("Invalid recognition settings")?;
        for width in [
            self.zones.left_width,
            self.zones.right_width,
            self.zones.top_height,
            self.zones.bottom_height,
        ] {
            ensure!(
                width.is_finite() && (0.0..=0.5).contains(&width),
                "Zone widths must be between 0 and 0.5"
            );
        }
        for actions in self.gestures.values() {
            for action in actions.values() {
                ensure!(
                    action.action == "command" && !action.cmd.trim().is_empty(),
                    "Legacy gestures require action = command and a nonempty cmd"
                );
            }
        }
        let mut seen = std::collections::HashSet::new();
        for (index, b) in self.bindings.iter().enumerate() {
            let label = format!(
                "binding {} ({:?}, {} fingers, {:?})",
                index + 1,
                b.zone,
                b.fingers,
                b.gesture
            );
            ensure!((1..=4).contains(&b.fingers), "{label}: fingers must be 1–4");
            ensure!(
                seen.insert((b.zone, b.fingers, b.gesture)),
                "{label}: duplicate binding"
            );
            ensure!(
                b.action == "command" && !b.cmd.trim().is_empty(),
                "{label}: requires action = command and a nonempty cmd"
            );
            let tap = matches!(b.gesture, Gesture::Tap | Gesture::DoubleTap);
            let hold = b.gesture == Gesture::Hold;
            let slide = matches!(
                b.gesture,
                Gesture::SlideUp | Gesture::SlideDown | Gesture::SlideLeft | Gesture::SlideRight
            );
            ensure!(
                b.tap_distance.is_none() || tap || hold,
                "{label}: tap_distance only applies to taps and holds"
            );
            ensure!(
                b.tap_ms.is_none() || tap,
                "{label}: tap_ms only applies to taps"
            );
            ensure!(
                b.double_tap_ms.is_none() || b.gesture == Gesture::DoubleTap,
                "{label}: double_tap_ms only applies to double taps"
            );
            ensure!(
                b.hold_ms.is_none() || hold,
                "{label}: hold_ms only applies to holds"
            );
            ensure!(
                b.step_distance.is_none() || slide,
                "{label}: step_distance only applies to slides"
            );
            ensure!(
                b.swipe_distance.is_none() || (!tap && !hold && !slide),
                "{label}: swipe_distance only applies to swipes"
            );
            settings(&b.settings(&self.recognition)).with_context(|| label)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_and_explicit_override() {
        let text = "[gestures.left.up]\naction='command'\ncmd='legacy'\n[[bindings]]\nzone='left'\nfingers=1\ngesture='swipe_up'\naction='command'\ncmd='override'\nswipe_distance=0.1\n";
        let c: Config = toml::from_str(text).unwrap();
        c.validate().unwrap();
        let (action, r) = c.binding(Zone::Left, 1, Gesture::SwipeUp).unwrap();
        assert_eq!(action.cmd, "override");
        assert_eq!(r.swipe_distance, 0.1);
        assert!(c.binding(Zone::Left, 2, Gesture::SwipeUp).is_none());
        let c: Config = toml::from_str(include_str!("../config.toml.example")).unwrap();
        c.validate().unwrap();
        assert!(c.binding(Zone::Left, 1, Gesture::SwipeUp).is_some());
        Config::default().validate().unwrap();
    }
    #[test]
    fn invalid_settings_and_bindings_are_rejected() {
        let binding =
            "[[bindings]]\nzone='left'\nfingers=2\ngesture='tap'\naction='command'\ncmd='true'\n";
        for text in [
            format!("{binding}{binding}"),
            binding.replace("fingers=2", "fingers=5"),
            format!("{binding}step_distance=0.1"),
            format!("{binding}tap_ms=0"),
            "[recognition]\nstep_distance=nan".into(),
            "[recognition]\nhold_ms=40".into(),
            "[zones]\nleft_width=-0.1".into(),
            binding.replace("cmd='true'", "cmd=''"),
            binding.replace("action='command'", "action='unknown'"),
        ] {
            assert!(
                toml::from_str::<Config>(&text).unwrap().validate().is_err(),
                "{text}"
            );
        }
        assert!(toml::from_str::<Config>(&format!("{binding}unknown=1")).is_err());
    }
}
