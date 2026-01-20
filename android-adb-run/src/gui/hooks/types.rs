use crate::adb::AdbBackend;
use crate::game_automation::AutomationCommand;
use crate::game_automation::GameState;
use crate::game_automation::types::TimedEvent;
use dioxus::prelude::Signal;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Device info tuple: (device_name, transport_id, screen_width, screen_height)
pub type DeviceInfoTuple = (String, Option<u32>, u32, u32);

/// Shared ADB client backend
pub type SharedAdbClient = Signal<Option<Arc<Mutex<AdbBackend>>>>;

/// Automation command sender
pub type CommandTxSignal = Signal<Option<tokio::sync::mpsc::Sender<AutomationCommand>>>;

// ============================================================================
// GROUPED SIGNAL STRUCTS - Reduce parameter explosion in function signatures
// ============================================================================

/// Screenshot-related signals grouped together
#[derive(Clone, Copy)]
pub struct ScreenshotSignals {
    pub data: Signal<Option<String>>,   // Base64 encoded for display
    pub bytes: Signal<Option<Vec<u8>>>, // Raw bytes for processing
    pub status: Signal<String>,         // Status message
    pub status_history: Signal<Vec<(String, bool)>>, // Status message history with flag: (message, is_result)
    pub counter: Signal<u64>,                        // Screenshot counter
    pub is_loading: Signal<bool>,                    // Loading indicator
    pub matched_patch: Signal<Option<String>>,       // Latest matched patch name
}

/// Device connection signals grouped together
#[derive(Clone, Copy)]
pub struct DeviceSignals {
    pub info: Signal<Option<DeviceInfoTuple>>, // Device info tuple
    pub status: Signal<String>,                // Connection status
    pub coords: Signal<Option<(u32, u32)>>,    // Current device coordinates
}

/// Automation state signals grouped together
#[derive(Clone, Copy)]
pub struct AutomationStateSignals {
    pub state: Signal<GameState>,                     // Running/Paused/Idle
    pub command_tx: CommandTxSignal,                  // Command sender
    pub is_paused_by_touch: Signal<bool>,             // Touch pause state
    pub touch_timeout_remaining: Signal<Option<u64>>, // Seconds until resume
    pub timed_tap_countdown: Signal<Option<(String, u64)>>, // Current countdown
    pub timed_events_list: Signal<Vec<TimedEvent>>,   // All timed events
}

/// User interaction signals grouped together
#[derive(Clone, Copy)]
pub struct InteractionSignals {
    pub mouse_coords: Signal<Option<(i32, i32)>>,
    pub auto_update_on_touch: Signal<bool>,
    pub select_box: Signal<bool>,
    pub is_swiping: Signal<bool>,
    pub swipe_start: Signal<Option<(u32, u32)>>,
    pub swipe_end: Signal<Option<(u32, u32)>>,
    pub selection_start: Signal<Option<dioxus::html::geometry::ElementPoint>>,
    pub selection_end: Signal<Option<dioxus::html::geometry::ElementPoint>>,
    pub hover_tap_preview: Signal<Option<(u32, u32)>>,
}

// ============================================================================
// LEGACY TYPE ALIASES - Kept for backward compatibility during migration
// ============================================================================

/// Device info signal (legacy alias)
pub type DeviceInfoSignal = Signal<Option<DeviceInfoTuple>>;

/// Screenshot data (base64 encoded) - legacy alias
pub type ScreenshotDataSignal = Signal<Option<String>>;

/// Screenshot bytes - legacy alias
pub type ScreenshotBytesSignal = Signal<Option<Vec<u8>>>;

/// Timed event list - legacy alias
pub type TimedEventsSignal = Signal<Vec<TimedEvent>>;

/// Timed countdown - legacy alias
pub type TimedCountdownSignal = Signal<Option<(String, u64)>>;

/// Touch timeout remaining - legacy alias
pub type TouchTimeoutSignal = Signal<Option<u64>>;
