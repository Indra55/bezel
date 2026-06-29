use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::gesture::{Direction, Zone};

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
        left_gestures.insert(Direction::Up, GestureAction { action: "command".into(), cmd: "wpctl set-volume @DEFAULT_SINK@ 5%+".into() });
        left_gestures.insert(Direction::Down, GestureAction { action: "command".into(), cmd: "wpctl set-volume @DEFAULT_SINK@ 5%-".into() });
        gestures.insert(Zone::Left, left_gestures);

        let mut right_gestures = HashMap::new();
        right_gestures.insert(Direction::Up, GestureAction { action: "command".into(), cmd: "brightnessctl set 10%+".into() });
        right_gestures.insert(Direction::Down, GestureAction { action: "command".into(), cmd: "brightnessctl set 10%-".into() });
        gestures.insert(Zone::Right, right_gestures);

        let mut top_gestures = HashMap::new();
        top_gestures.insert(Direction::Left, GestureAction { action: "command".into(), cmd: "hyprctl dispatch workspace e-1".into() });
        top_gestures.insert(Direction::Right, GestureAction { action: "command".into(), cmd: "hyprctl dispatch workspace e+1".into() });
        gestures.insert(Zone::Top, top_gestures);

        let mut bottom_gestures = HashMap::new();
        bottom_gestures.insert(Direction::Left, GestureAction { action: "command".into(), cmd: "playerctl previous".into() });
        bottom_gestures.insert(Direction::Right, GestureAction { action: "command".into(), cmd: "playerctl next".into() });
        bottom_gestures.insert(Direction::Tap, GestureAction { action: "command".into(), cmd: "playerctl play-pause".into() });
        gestures.insert(Zone::Bottom, bottom_gestures);

        Config {
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

pub fn load_config() -> Result<Config> {
    let path = get_config_path();
    if !path.exists() {
        tracing::warn!("Config file not found at {:?}, using defaults", path);
        return Ok(Config::default());
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file at {:?}", path))?;
    let config: Config = toml::from_str(&content).context("Failed to parse config.toml")?;
    
    Ok(config)
}
