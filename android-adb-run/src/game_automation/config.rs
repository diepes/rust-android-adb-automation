use super::types::{MAX_TAP_INTERVAL_SECONDS, MIN_TAP_INTERVAL_SECONDS, TimedEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use time::OffsetDateTime;

pub const TIMED_EVENTS_CONFIG_PATH: &str = "conf_timed_events.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapEventConfig {
    pub id: String,
    pub x: u32,
    pub y: u32,
    pub interval_seconds: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimedEventsConfig {
    pub screenshot_interval_minutes: u64,
    pub countdown_interval_seconds: u64,
    pub taps: Vec<TapEventConfig>,
}

impl Default for TimedEventsConfig {
    fn default() -> Self {
        Self {
            screenshot_interval_minutes: 10,
            countdown_interval_seconds: 1,
            taps: vec![
                TapEventConfig {
                    id: "claim_5d_tap".to_string(),
                    x: 120,
                    y: 1250,
                    interval_seconds: 60,
                    enabled: true,
                },
                TapEventConfig {
                    id: "restart_tap".to_string(),
                    x: 110,
                    y: 1600,
                    interval_seconds: 120,
                    enabled: true,
                },
                TapEventConfig {
                    id: "claim_1d_tap".to_string(),
                    x: 350,
                    y: 628,
                    interval_seconds: 15,
                    enabled: true,
                },
            ],
        }
    }
}

pub fn load_or_create_timed_events(debug_enabled: bool) -> HashMap<String, TimedEvent> {
    let path = Path::new(TIMED_EVENTS_CONFIG_PATH);
    match load_or_create_config(path) {
        Ok(config) => build_timed_events(config),
        Err(ConfigLoadError::InvalidConfig(error)) => {
            eprintln!("❌ Invalid timed events config ({}). Please fix {} and restart.", error, path.display());
            std::process::exit(1);
        }
        Err(error) => {
            debug_print!(
                debug_enabled,
                "⚠️ Timed events config error ({}), using defaults",
                error
            );
            build_timed_events(TimedEventsConfig::default())
        }
    }
}

enum ConfigLoadError {
    InvalidConfig(String),
    Other(String),
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(f, "{}", message),
            Self::Other(message) => write!(f, "{}", message),
        }
    }
}

fn load_or_create_config(path: &Path) -> Result<TimedEventsConfig, ConfigLoadError> {
    if !path.exists() {
        let default_config = TimedEventsConfig::default();
        let serialized = toml::to_string_pretty(&default_config)
            .map_err(|e| ConfigLoadError::Other(format!("Failed to serialize default timed events config: {}", e)))?;
        let header = format!(
            "# Default config created {} feel free to edit\n\n",
            OffsetDateTime::now_utc().date()
        );

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .map_err(|e| ConfigLoadError::Other(format!("Failed to create config directory: {}", e)))?;
        }

        fs::write(path, format!("{}{}", header, serialized))
            .map_err(|e| ConfigLoadError::Other(format!("Failed to write timed events config file: {}", e)))?;

        println!(
            "🆕 Created {} with {} timed events",
            path.display(),
            default_config.taps.len()
        );

        return Ok(default_config);
    }

    let content = fs::read_to_string(path)
        .map_err(|e| ConfigLoadError::Other(format!("Failed to read timed events config file: {}", e)))?;
    let config = toml::from_str::<TimedEventsConfig>(&content)
        .map_err(|e| ConfigLoadError::InvalidConfig(format!("Failed to parse timed events config file: {}", e)))?;

    println!(
        "📥 Loaded {} timed events from {}",
        config.taps.len(),
        path.display()
    );

    Ok(config)
}

fn build_timed_events(config: TimedEventsConfig) -> HashMap<String, TimedEvent> {
    let mut timed_events = HashMap::new();

    timed_events.insert(
        "screenshot".to_string(),
        TimedEvent::new_screenshot_minutes(config.screenshot_interval_minutes),
    );
    timed_events.insert(
        "countdown_update".to_string(),
        TimedEvent::new_countdown_update(config.countdown_interval_seconds),
    );

    for tap in config.taps {
        let interval_seconds = tap
            .interval_seconds
            .clamp(MIN_TAP_INTERVAL_SECONDS, MAX_TAP_INTERVAL_SECONDS);

        let mut event = TimedEvent::new_tap_seconds(tap.id.clone(), tap.x, tap.y, interval_seconds);
        event.enabled = tap.enabled;
        timed_events.insert(tap.id, event);
    }

    timed_events
}
