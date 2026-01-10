use crate::adb::AdbBackend;
use crate::game_automation::AutomationCommand;
use crate::game_automation::types::TimedEvent;
use dioxus::prelude::Signal;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Device info tuple: (device_name, transport_id, screen_width, screen_height)
pub type DeviceInfoSignal = Signal<Option<(String, Option<u32>, u32, u32)>>;

/// Shared ADB client backend
pub type SharedAdbClient = Signal<Option<Arc<Mutex<AdbBackend>>>>;

/// Automation command sender
pub type CommandTxSignal = Signal<Option<tokio::sync::mpsc::Sender<AutomationCommand>>>;

/// Screenshot data (base64 encoded)
pub type ScreenshotDataSignal = Signal<Option<String>>;

/// Screenshot bytes
pub type ScreenshotBytesSignal = Signal<Option<Vec<u8>>>;

/// Timed event list
pub type TimedEventsSignal = Signal<Vec<TimedEvent>>;

/// Timed countdown
pub type TimedCountdownSignal = Signal<Option<(String, u64)>>;

/// Touch timeout remaining
pub type TouchTimeoutSignal = Signal<Option<u64>>;
