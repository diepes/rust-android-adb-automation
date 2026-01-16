// Finite State Machine implementation for game automation - Event Driven Architecture
use super::match_image::{GameStateDetector, MatchConfig, create_default_config};
use super::types::{
    AutomationCommand, DeviceInfo, GameState, MAX_TAP_INTERVAL_SECONDS, MIN_TAP_INTERVAL_SECONDS,
    TimedEvent, TimedEventType,
};
use crate::adb::{AdbBackend, AdbClient};
use dioxus::prelude::{Signal, WritableExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, timeout};

// Helper function to detect if an error indicates device disconnection
// NOTE: This is careful to avoid false positives from normal cleanup operations
pub fn is_disconnect_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();
    
    // IMPORTANT: "no write endpoint setup" during CLSE cleanup is NOT a disconnect
    // Only treat it as disconnect if it happens during actual operations
    // We filter out CLSE messages which are harmless cleanup operations
    let is_clse_cleanup_error = error_lower.contains("clse") || 
                                (error_lower.contains("no write endpoint") && 
                                 error_lower.contains("error while sending"));
    
    if is_clse_cleanup_error {
        // These are harmless cleanup errors from the USB transport layer
        // They don't indicate actual device disconnection
        return false;
    }
    
    // Clear device disconnection indicators (actual problems)
    error_lower.contains("device offline")
        || error_lower.contains("device not found")
        || error_lower.contains("not found")           // Catches "device 'xxx' not found"
        || error_lower.contains("no devices")
        || error_lower.contains("emulators found")
        || error_lower.contains("connection refused")
        || error_lower.contains("broken pipe")
        || error_lower.contains("connection reset")
        || error_lower.contains("closed")
        || error_lower.contains("not connected")
        || (error_lower.contains("timed out") && error_lower.contains("usb")) // USB timeout = disconnect
        || error_lower.contains("operation timed out") // Only if consistent failures
        || (error_lower.contains("usb") && error_lower.contains("error") && !error_lower.contains("resource busy")) // Generic USB error but not resource busy
}

pub struct GameAutomation {
    state: GameState,
    adb_client: Option<Arc<Mutex<AdbBackend>>>,
    command_rx: mpsc::Receiver<AutomationCommand>,
    is_running: bool,
    should_exit: bool,
    debug_enabled: bool,
    // New image matching system
    latest_screenshot: Option<Vec<u8>>, // Raw PNG bytes
    game_detector: GameStateDetector,
    // Unified timed events system
    timed_events: HashMap<String, TimedEvent>,
    // Reconnection tracking
    last_reconnect_attempt: Option<std::time::Instant>,
    device_disconnected: bool,
    // Direct signal updates (replacing event channel)
    screenshot_data: Signal<Option<String>>,
    screenshot_bytes: Signal<Option<Vec<u8>>>,
    screenshot_status: Signal<String>,
    automation_state: Signal<GameState>,
    is_paused_by_touch: Signal<bool>,
    touch_timeout_remaining: Signal<Option<u64>>,
    timed_tap_countdown: Signal<Option<(String, u64)>>,
    timed_events_list: Signal<Vec<TimedEvent>>,
    device_info: Signal<Option<DeviceInfo>>,
    status: Signal<String>,
    screenshot_counter: Signal<u64>,
}

impl GameAutomation {
    pub fn new(
        command_rx: mpsc::Receiver<AutomationCommand>,
        debug_enabled: bool,
        signals: super::types::AutomationSignals,
    ) -> Self {
        // Create default detector (will be updated with screen dimensions later)
        let config = create_default_config();
        let game_detector = GameStateDetector::new(1080, 2400, config); // Default dimensions

        let mut timed_events = HashMap::new();

        // Create core system events
        timed_events.insert(
            "screenshot".to_string(),
            TimedEvent::new_screenshot_minutes(10), // 10 minutes between screenshots
        );
        timed_events.insert(
            "countdown_update".to_string(),
            TimedEvent::new_countdown_update(1), // Every 1 second for countdown updates
        );

        // Define timed taps with flexible intervals
        // Format: (id, x, y, interval_type, interval_value)
        let tap_definitions = vec![
            ("claim_5d_tap", 120, 1250, "minutes", 1), // Every 2 minutes
            ("restart_tap", 110, 1600, "minutes", 2),  // Every 9 minutes
            ("claim_1d_tap", 350, 628, "seconds", 15), // Every 15sec
                                                       // Add more taps here as needed with seconds or minutes
        ];

        // Create and insert all timed tap events with flexible intervals
        for (id, x, y, interval_type, interval_value) in tap_definitions {
            let event = match interval_type {
                "seconds" => TimedEvent::new_tap_seconds(id.to_string(), x, y, interval_value),
                "minutes" => TimedEvent::new_tap_minutes(id.to_string(), x, y, interval_value),
                _ => TimedEvent::new_tap_minutes(id.to_string(), x, y, interval_value), // Default to minutes
            };
            timed_events.insert(id.to_string(), event);
        }

        // Example: Create a custom event with precise Duration (e.g., 2.5 minutes)
        // let custom_event = TimedEvent::new(
        //     "custom_tap".to_string(),
        //     TimedEventType::Tap { x: 500, y: 500 },
        //     Duration::from_secs(150), // 2.5 minutes = 150 seconds
        // );
        // timed_events.insert("custom_tap".to_string(), custom_event);

        if debug_enabled {
            println!("🕒 Initialized {} timed events:", timed_events.len());
            for (id, event) in &timed_events {
                match &event.event_type {
                    TimedEventType::Screenshot => {
                        println!("  - {}: Screenshot every {}s", id, event.interval.as_secs());
                    }
                    TimedEventType::Tap { x, y } => {
                        println!(
                            "  - {}: Tap at ({},{}) every {}min",
                            id,
                            x,
                            y,
                            event.interval.as_secs() / 60
                        );
                    }
                    TimedEventType::CountdownUpdate => {
                        println!(
                            "  - {}: Countdown update every {}s",
                            id,
                            event.interval.as_secs()
                        );
                    }
                }
            }
        }

        Self {
            state: GameState::Idle,
            adb_client: None,
            command_rx,
            is_running: false,
            should_exit: false,
            debug_enabled,
            latest_screenshot: None,
            game_detector,
            timed_events,
            last_reconnect_attempt: None,
            device_disconnected: false,
            screenshot_data: signals.screenshot_data,
            screenshot_bytes: signals.screenshot_bytes,
            screenshot_status: signals.screenshot_status,
            automation_state: signals.automation_state,
            is_paused_by_touch: signals.is_paused_by_touch,
            touch_timeout_remaining: signals.touch_timeout_remaining,
            timed_tap_countdown: signals.timed_tap_countdown,
            timed_events_list: signals.timed_events_list,
            device_info: signals.device_info,
            status: signals.status,
            screenshot_counter: signals.screenshot_counter,
        }
    }

    /// Accept a shared ADB client reference (prevents creating multiple USB connections)
    pub async fn set_shared_adb_client(
        &mut self,
        shared_client: Arc<Mutex<AdbBackend>>,
    ) -> Result<(), String> {
        // Get screen dimensions from the shared client
        let (screen_width, screen_height) = {
            let client_guard = shared_client.lock().await;
            client_guard.screen_dimensions()
        };

        // Update detector with actual screen dimensions
        let mut config = create_default_config();
        config.debug_enabled = self.debug_enabled;
        self.game_detector = GameStateDetector::new(screen_width, screen_height, config);

        // Load templates from current directory
        match self.game_detector.load_templates(".") {
            Ok(count) => {
                debug_print!(
                    self.debug_enabled,
                    "✅ Loaded {} templates for game state detection",
                    count
                );
                // Templates loaded - no GUI notification needed (templates are internal)
            }
            Err(e) => {
                debug_print!(self.debug_enabled, "⚠️ Template loading warning: {}", e);
            }
        }

        // Use the shared connection directly (no new Arc creation)
        self.adb_client = Some(shared_client);

        // Start touch monitoring for automatic pause/resume
        if let Some(client_arc) = &self.adb_client {
            let client_guard = client_arc.lock().await;
            if let Err(e) = client_guard.start_touch_monitoring().await {
                debug_print!(
                    self.debug_enabled,
                    "⚠️ Failed to start touch monitoring: {}",
                    e
                );
            } else {
                debug_print!(
                    self.debug_enabled,
                    "👆 Touch monitoring started (30s timeout)"
                );
            }
        }
        Ok(())
    }

    /// Accept an owned ADB client (creates new Arc - prefer set_shared_adb_client for USB)
    pub async fn set_adb_client(&mut self, client: AdbBackend) -> Result<(), String> {
        self.set_shared_adb_client(Arc::new(Mutex::new(client)))
            .await
    }

    /// Legacy method - kept for backward compatibility (creates new connection)
    pub async fn initialize_adb(&mut self) -> Result<(), String> {
        match AdbBackend::connect_first().await {
            Ok(client) => {
                self.set_adb_client(client).await?;
                debug_print!(self.debug_enabled, "🤖 Game automation initialized");
                Ok(())
            }
            Err(e) => {
                let error = format!("Failed to initialize ADB for automation: {}", e);
                *self.screenshot_status.write_unchecked() = format!("❌ {}", error);
                Err(error)
            }
        }
    }

    async fn change_state(&mut self, new_state: GameState) {
        if self.state != new_state {
            debug_print!(
                self.debug_enabled,
                "🎮 Game automation state: {:?} -> {:?}",
                self.state,
                new_state
            );
            self.state = new_state.clone();
            *self.automation_state.write_unchecked() = new_state;
        }
    }

    async fn take_screenshot(&mut self) -> Result<Vec<u8>, String> {
        let start_time = std::time::Instant::now();

        if let Some(client) = &self.adb_client {
            let (screenshot_result, duration_ms) = {
                let client_guard = client.lock().await;
                let result = client_guard.screen_capture_bytes().await;
                let duration = start_time.elapsed().as_millis();
                (result, duration)
            }; // Lock released here

            match screenshot_result {
                Ok(bytes) => {
                    debug_print!(
                        self.debug_enabled,
                        "📸 Game automation captured screenshot ({} bytes) in {}ms",
                        bytes.len(),
                        duration_ms
                    );

                    // Store the latest screenshot for image recognition
                    self.latest_screenshot = Some(bytes.clone());

                    // Update screenshot signals directly
                    let bytes_for_encoding = bytes.clone();
                    let bytes_for_signal = bytes.clone();
                    let mut screenshot_counter_clone = self.screenshot_counter;
                    let screenshot_data_clone = self.screenshot_data;
                    let screenshot_bytes_clone = self.screenshot_bytes;
                    let screenshot_status_clone = self.screenshot_status;

                    // Spawn base64 encoding in background to avoid blocking
                    dioxus::prelude::spawn(async move {
                        use crate::gui::util::base64_encode;
                        let counter_val = screenshot_counter_clone.with_mut(|c| {
                            *c += 1;
                            *c
                        });
                        let base64_string =
                            tokio::task::spawn_blocking(move || base64_encode(&bytes_for_encoding))
                                .await
                                .unwrap_or_default();
                        *screenshot_data_clone.write_unchecked() = Some(base64_string);
                        *screenshot_bytes_clone.write_unchecked() = Some(bytes_for_signal);
                        *screenshot_status_clone.write_unchecked() = format!(
                            "🤖 Automation screenshot #{} ({}ms)",
                            counter_val, duration_ms
                        );
                    });

                    Ok(bytes)
                }
                Err(e) => {
                    let error = format!("Screenshot failed: {}", e);

                    // Check if this is a disconnect error
                    if is_disconnect_error(&error) {
                        println!("🔌 Device disconnect detected: {}", error);
                        self.device_disconnected = true;
                        self.last_reconnect_attempt = None; // Reset for immediate reconnection attempt
                        *self.device_info.write_unchecked() = None;
                        *self.screenshot_data.write_unchecked() = None;
                        *self.screenshot_bytes.write_unchecked() = None;
                        *self.screenshot_status.write_unchecked() =
                            format!("🔌 USB DISCONNECTED: {} - Please reconnect", error);
                        *self.status.write_unchecked() =
                            "🔌 Device Disconnected - Paused".to_string();
                    } else {
                        *self.screenshot_status.write_unchecked() =
                            format!("🤖 Automation error: {}", error);
                    }
                    Err(error)
                }
            }
        } else {
            Err("ADB client not initialized".to_string())
        }
    }

    async fn process_command(&mut self, command: AutomationCommand) {
        debug_print!(
            self.debug_enabled,
            "🤖 Processing automation command: {:?}",
            command
        );
        match command {
            AutomationCommand::Start => {
                debug_print!(
                    self.debug_enabled,
                    "🤖 Start command received. Current is_running: {}",
                    self.is_running
                );
                if !self.is_running {
                    // Check if ADB client is available before starting
                    if self.adb_client.is_none() {
                        println!("⚠️ Cannot start automation: ADB client not initialized");
                        return;
                    }

                    self.is_running = true;
                    self.change_state(GameState::Running).await;
                    println!(
                        "🚀 Game automation STARTED. is_running={}, state={:?}",
                        self.is_running, self.state
                    );

                    debug_print!(
                        self.debug_enabled,
                        "🚀 Game automation started. Timed events: {} configured",
                        self.timed_events.len()
                    );

                    // List timed events on start for debugging
                    if self.debug_enabled {
                        for (id, event) in &self.timed_events {
                            let next_in = event
                                .time_until_next()
                                .map(|d| format!("{:.1}s", d.as_secs_f32()))
                                .unwrap_or_else(|| "disabled".to_string());
                            debug_print!(
                                self.debug_enabled,
                                "  🕒 Timed event '{}': {:?} every {}s, next in {}",
                                id,
                                event.event_type,
                                event.interval.as_secs(),
                                next_in
                            );
                        }
                    }

                    // Send initial events list and countdown information to GUI immediately
                    self.send_timed_events_list().await;
                    self.send_timed_tap_countdowns().await;
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "🤖 Automation already running, ignoring start command"
                    );
                }
            }
            AutomationCommand::Pause => {
                if self.is_running {
                    self.change_state(GameState::Paused).await;
                    debug_print!(self.debug_enabled, "⏸️ Game automation paused");
                }
            }
            AutomationCommand::Resume => {
                if self.is_running && self.state == GameState::Paused {
                    self.change_state(GameState::Running).await;
                    debug_print!(self.debug_enabled, "▶️ Game automation resumed");

                    // Send current events list and countdown information to GUI immediately after resume
                    self.send_timed_events_list().await;
                    self.send_timed_tap_countdowns().await;
                }
            }
            AutomationCommand::Stop => {
                self.is_running = false;

                // Stop touch monitoring when automation stops
                if let Some(client_arc) = &self.adb_client {
                    let client_guard = client_arc.lock().await;
                    if let Err(e) = client_guard.stop_touch_monitoring().await {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ Failed to stop touch monitoring: {}",
                            e
                        );
                    } else {
                        debug_print!(self.debug_enabled, "👆 Touch monitoring stopped");
                    }
                }

                self.change_state(GameState::Idle).await;
                debug_print!(self.debug_enabled, "⏹️ Game automation stopped");
            }
            AutomationCommand::ClearTouchActivity => {
                // Clear touch activity to resume automation immediately
                if let Some(client_arc) = &self.adb_client {
                    let client_guard = client_arc.lock().await;
                    if let Err(e) = client_guard.clear_touch_activity().await {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ Failed to clear touch activity: {}",
                            e
                        );
                    } else {
                        debug_print!(
                            self.debug_enabled,
                            "👆 Touch activity cleared - automation resuming"
                        );
                        // Update GUI signals
                        *self.is_paused_by_touch.write_unchecked() = false;
                        *self.touch_timeout_remaining.write_unchecked() = None;
                    }
                }
            }
            AutomationCommand::RegisterTouchActivity => {
                // Register touch activity to pause automation for 30 seconds
                if let Some(client_arc) = &self.adb_client {
                    let client_guard = client_arc.lock().await;
                    if let Err(e) = client_guard.register_touch_activity().await {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ Failed to register touch activity: {}",
                            e
                        );
                    } else {
                        debug_print!(
                            self.debug_enabled,
                            "👆 GUI touch registered - pausing automation for 30s"
                        );
                        // Update GUI signals with countdown
                        *self.is_paused_by_touch.write_unchecked() = true;
                        *self.touch_timeout_remaining.write_unchecked() = Some(30);
                    }
                }
            }
            AutomationCommand::TakeScreenshot => {
                // Allow manual screenshot at any time to detect disconnects
                if let Err(e) = self.take_screenshot().await {
                    debug_print!(self.debug_enabled, "❌ Manual screenshot failed: {}", e);
                }
            }
            AutomationCommand::TestImageRecognition => {
                debug_print!(
                    self.debug_enabled,
                    "🧪 Manual image recognition test requested"
                );
                if let Err(e) = self.test_image_recognition().await {
                    *self.screenshot_status.write_unchecked() = format!("❌ {}", e);
                }
            }
            AutomationCommand::RescanTemplates => {
                debug_print!(self.debug_enabled, "🔄 Template rescan requested");
                if let Err(e) = self.rescan_templates().await {
                    *self.screenshot_status.write_unchecked() = format!("❌ {}", e);
                }
            }
            AutomationCommand::AddTimedEvent(event) => {
                debug_print!(
                    self.debug_enabled,
                    "➕ Adding timed event '{}': {:?} every {}s",
                    event.id,
                    event.event_type,
                    event.interval.as_secs()
                );
                self.timed_events.insert(event.id.clone(), event);
            }
            AutomationCommand::RemoveTimedEvent(id) => {
                if self.timed_events.remove(&id).is_some() {
                    debug_print!(self.debug_enabled, "➖ Removed timed event '{}'", id);
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "⚠️ Timed event '{}' not found for removal",
                        id
                    );
                }
            }
            AutomationCommand::EnableTimedEvent(id) => {
                if let Some(event) = self.timed_events.get_mut(&id) {
                    event.enabled = true;
                    debug_print!(self.debug_enabled, "✅ Enabled timed event '{}'", id);
                    // Send updated events list to GUI
                    self.send_timed_events_list().await;
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "⚠️ Timed event '{}' not found for enabling",
                        id
                    );
                }
            }
            AutomationCommand::DisableTimedEvent(id) => {
                if let Some(event) = self.timed_events.get_mut(&id) {
                    event.enabled = false;
                    debug_print!(self.debug_enabled, "❌ Disabled timed event '{}'", id);
                    // Send updated events list to GUI
                    self.send_timed_events_list().await;
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "⚠️ Timed event '{}' not found for disabling",
                        id
                    );
                }
            }
            AutomationCommand::AdjustTimedEventInterval { id, delta_seconds } => {
                if let Some(event) = self.timed_events.get_mut(&id) {
                    if matches!(event.event_type, TimedEventType::Tap { .. }) {
                        let current_secs = event.interval.as_secs();
                        let current_secs_i64 = current_secs as i64;
                        let min_secs = MIN_TAP_INTERVAL_SECONDS as i64;
                        let max_secs = MAX_TAP_INTERVAL_SECONDS as i64;

                        let target_secs = current_secs_i64.saturating_add(delta_seconds);
                        let clamped_secs = target_secs.clamp(min_secs, max_secs) as u64;

                        if clamped_secs != current_secs {
                            event.set_interval(Duration::from_secs(clamped_secs));
                            debug_print!(
                                self.debug_enabled,
                                "⏱️ Adjusted timed event '{}' interval to {}s",
                                id,
                                clamped_secs
                            );
                            self.send_timed_events_list().await;
                            self.send_timed_tap_countdowns().await;
                        } else {
                            debug_print!(
                                self.debug_enabled,
                                "ℹ️ Timed event '{}' interval unchanged ({}s)",
                                id,
                                clamped_secs
                            );
                        }
                    } else {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ Interval adjustments only supported for tap events ({}).",
                            id
                        );
                    }
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "⚠️ Timed event '{}' not found for interval adjustment",
                        id
                    );
                }
            }
            AutomationCommand::TriggerTimedEvent(id) => {
                if let Some(event) = self.timed_events.get(&id) {
                    if event.enabled {
                        debug_print!(
                            self.debug_enabled,
                            "🔫 Triggering timed event '{}' immediately",
                            id
                        );
                        // Execute the event immediately
                        match event.event_type {
                            TimedEventType::Screenshot => {
                                let _ = self.take_screenshot().await;
                            }
                            TimedEventType::Tap { x, y } => {
                                if let Some(adb_client) = &self.adb_client {
                                    let client = adb_client.lock().await;
                                    if let Err(e) = client.tap(x, y).await {
                                        debug_print!(
                                            self.debug_enabled,
                                            "⚠️ Failed to execute tap ({}, {}): {}",
                                            x,
                                            y,
                                            e
                                        );
                                        // Check if this is a disconnect error
                                        if is_disconnect_error(&e.to_string()) {
                                            debug_print!(
                                                self.debug_enabled,
                                                "🔌 Device disconnect detected during manual tap trigger: {}",
                                                e
                                            );
                                            self.device_disconnected = true;
                                            self.last_reconnect_attempt = None;
                                            *self.device_info.write_unchecked() = None;
                                            *self.screenshot_data.write_unchecked() = None;
                                            *self.screenshot_bytes.write_unchecked() = None;
                                            *self.screenshot_status.write_unchecked() = format!(
                                                "🔌 USB DISCONNECTED: {} - Please reconnect",
                                                e
                                            );
                                            *self.status.write_unchecked() =
                                                "🔌 Device Disconnected - Paused".to_string();
                                        }
                                    } else {
                                        // Tap executed successfully - no GUI notification needed for manual triggers
                                    }
                                }
                            }
                            TimedEventType::CountdownUpdate => {
                                // Don't trigger countdown updates manually, they're system events
                                debug_print!(
                                    self.debug_enabled,
                                    "⚠️ Cannot manually trigger countdown update event"
                                );
                            }
                        }
                        // Mark as executed and send events list update
                        if let Some(event) = self.timed_events.get_mut(&id) {
                            event.mark_executed();
                        }
                        self.send_timed_events_list().await;
                        // Event executed successfully - list already updated
                    } else {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ Cannot trigger disabled event '{}'",
                            id
                        );
                    }
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "⚠️ Timed event '{}' not found for triggering",
                        id
                    );
                }
            }
            AutomationCommand::ListTimedEvents => {
                let events: Vec<TimedEvent> = self.timed_events.values().cloned().collect();
                debug_print!(
                    self.debug_enabled,
                    "📋 Listing {} timed events",
                    events.len()
                );
                for event in &events {
                    let status = if event.enabled { "enabled" } else { "disabled" };
                    let next_time = match event.time_until_next() {
                        Some(duration) => format!("{:.1}s", duration.as_secs_f32()),
                        None => "disabled".to_string(),
                    };
                    debug_print!(
                        self.debug_enabled,
                        "  - {}: {:?} every {}s, {}, next in {}",
                        event.id,
                        event.event_type,
                        event.interval.as_secs(),
                        status,
                        next_time
                    );
                }
                *self.timed_events_list.write_unchecked() = events;
            }
            AutomationCommand::Shutdown => {
                self.should_exit = true;
                self.is_running = false;
                self.change_state(GameState::Idle).await;
                println!("🛑 Game automation shutting down");
            }
        }
    }

    /// New event-driven main loop with timeout-based command handling
    pub async fn run(&mut self) {
        debug_print!(self.debug_enabled, "🎮 Event-driven automation FSM started");
        println!("🎮 Automation run() loop starting");

        let mut loop_count = 0u32;
        loop {
            loop_count += 1;
            if loop_count.is_multiple_of(10) {
                debug_print!(
                    self.debug_enabled,
                    "💓 Loop alive: {}, is_running={}",
                    loop_count,
                    self.is_running
                );
            }

            // Wait for commands with a 1-second timeout for responsive event processing
            match timeout(Duration::from_secs(1), self.command_rx.recv()).await {
                Ok(Some(command)) => {
                    // Process command immediately when received
                    self.process_command(command).await;
                }
                Ok(None) => {
                    // Channel closed, exit gracefully
                    debug_print!(self.debug_enabled, "🔌 Command channel closed");
                    break;
                }
                Err(_) => {
                    // Timeout occurred, continue to process timed events
                }
            }

            // Check for reconnection if device is disconnected
            if self.device_disconnected {
                self.check_reconnection().await;
            }

            // Process timed events if automation is running and not paused
            if self.is_running && self.state != GameState::Paused {
                self.process_timed_events().await;
            } else {
                static ONCE: std::sync::Once = std::sync::Once::new();
                ONCE.call_once(|| {
                    println!(
                        "⚠️ NOT processing events: is_running={}, state={:?}",
                        self.is_running, self.state
                    );
                });
            }

            // Break the loop if shutdown was requested
            if self.should_exit {
                break;
            }
        }

        debug_print!(self.debug_enabled, "🎮 Event-driven automation FSM ended");
    }

    /// Process all ready timed events (pauses if human is touching device)
    async fn process_timed_events(&mut self) {
        // Check if human is currently touching the device
        if let Some(client) = &self.adb_client {
            let (human_touching, remaining_seconds) = {
                let client_guard = client.lock().await;
                let touching = client_guard.is_human_touching().await;
                let remaining = if touching {
                    client_guard.get_touch_timeout_remaining().await
                } else {
                    None
                };
                (touching, remaining)
            }; // Lock released here

            if human_touching {
                debug_print!(
                    self.debug_enabled,
                    "🚫 AUTOMATION PAUSED: Human touch detected - skipping timed events"
                );
                // Update GUI signals with countdown
                *self.is_paused_by_touch.write_unchecked() = true;
                *self.touch_timeout_remaining.write_unchecked() = remaining_seconds;
                return; // Skip processing timed events while human is touching
            } else {
                // Only send "no activity" notification if we haven't sent it recently
                use std::sync::LazyLock;
                use std::sync::Mutex as StdMutex;

                static LAST_NO_ACTIVITY_SENT: LazyLock<StdMutex<std::time::Instant>> =
                    LazyLock::new(|| StdMutex::new(std::time::Instant::now()));

                let should_send = {
                    let mut last_sent = LAST_NO_ACTIVITY_SENT.lock().unwrap();
                    let elapsed = last_sent.elapsed().as_secs();
                    if elapsed > 5 {
                        // Send at most every 5 seconds
                        *last_sent = std::time::Instant::now();
                        true
                    } else {
                        false
                    }
                };

                if should_send {
                    debug_print!(
                        self.debug_enabled,
                        "✅ AUTOMATION ACTIVE: No human touch detected - processing events"
                    );
                    *self.is_paused_by_touch.write_unchecked() = false;
                    *self.touch_timeout_remaining.write_unchecked() = None;
                }
            }
        }

        let mut events_to_execute = Vec::new();

        // Collect ready events
        for (id, event) in &self.timed_events {
            if event.is_ready(self.debug_enabled) {
                debug_print!(self.debug_enabled, "✓ Event '{}' is READY", id);
                events_to_execute.push((id.clone(), event.event_type.clone()));
            }
        }

        // Sort events: Screenshot first, then CountdownUpdate, then Taps
        // This ensures screenshot doesn't compete with tap processor for USB lock
        events_to_execute.sort_by(|a, b| {
            let order_a = match a.1 {
                TimedEventType::Screenshot => 0,
                TimedEventType::CountdownUpdate => 1,
                TimedEventType::Tap { .. } => 2,
            };
            let order_b = match b.1 {
                TimedEventType::Screenshot => 0,
                TimedEventType::CountdownUpdate => 1,
                TimedEventType::Tap { .. } => 2,
            };
            order_a.cmp(&order_b)
        });

        if events_to_execute.is_empty() {
            // Check why no events are ready
            static EMPTY_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let count = EMPTY_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count.is_multiple_of(30) && count > 0 {
                println!("📭 No events ready (checked {} times)", count);
                for (id, event) in &self.timed_events {
                    if let Some(last) = event.last_executed {
                        let elapsed = last.elapsed();
                        println!(
                            "  - {}: elapsed={:?} vs interval={:?}",
                            id, elapsed, event.interval
                        );
                    } else {
                        println!("  - {}: never executed", id);
                    }
                }
            }
        }

        // Execute ready events
        for (event_id, event_type) in events_to_execute {
            if let Err(e) = self.execute_timed_event(&event_id, &event_type).await {
                debug_print!(
                    self.debug_enabled,
                    "❌ Timed event '{}' failed: {}",
                    event_id,
                    e
                );

                // Check if this is a disconnect error
                if is_disconnect_error(&e.to_string()) {
                    debug_print!(
                        self.debug_enabled,
                        "🔌 Device disconnect detected during timed event: {}",
                        e
                    );

                    // Pause the automation
                    self.change_state(GameState::Paused).await;
                    self.device_disconnected = true;
                    self.last_reconnect_attempt = None;

                    // Update disconnect signals
                    *self.device_info.write_unchecked() = None;
                    *self.screenshot_data.write_unchecked() = None;
                    *self.screenshot_bytes.write_unchecked() = None;
                    *self.screenshot_status.write_unchecked() =
                        format!("🔌 USB DISCONNECTED: {} - Please reconnect", e);
                    *self.status.write_unchecked() = "🔌 Device Disconnected - Paused".to_string();
                    return; // Stop processing further events on disconnect
                } else {
                    *self.screenshot_status.write_unchecked() =
                        format!("❌ Timed event '{}' failed: {}", event_id, e);
                }
            }
        }
    }

    /// Execute a specific timed event
    async fn execute_timed_event(
        &mut self,
        event_id: &str,
        event_type: &TimedEventType,
    ) -> Result<(), String> {
        debug_print!(
            self.debug_enabled,
            "⚡ Executing timed event '{}': {:?}",
            event_id,
            event_type
        );

        match event_type {
            TimedEventType::Screenshot => {
                // Screenshot can take time and may contend for USB lock with tap processor
                // Run asynchronously to avoid blocking the automation loop

                // Take screenshot asynchronously to avoid blocking the automation loop
                // Note: We don't await here to prevent blocking, screenshot updates signals when done
                if let Some(client) = &self.adb_client {
                    let client_clone = client.clone();
                    let screenshot_data = self.screenshot_data;
                    let screenshot_bytes_sig = self.screenshot_bytes;
                    let screenshot_status = self.screenshot_status;
                    let mut screenshot_counter = self.screenshot_counter;

                    dioxus::prelude::spawn(async move {
                        let start = std::time::Instant::now();
                        match timeout(Duration::from_secs(10), async {
                            let guard = client_clone.lock().await;
                            guard.screen_capture_bytes().await
                        })
                        .await
                        {
                            Ok(Ok(bytes)) => {
                                let duration_ms = start.elapsed().as_millis();
                                let counter_val = screenshot_counter.with_mut(|c| {
                                    *c += 1;
                                    *c
                                });

                                // Encode and update signals in background
                                let bytes_clone = bytes.clone();
                                dioxus::prelude::spawn(async move {
                                    use crate::gui::util::base64_encode;
                                    let base64_string = tokio::task::spawn_blocking(move || {
                                        base64_encode(&bytes_clone)
                                    })
                                    .await
                                    .unwrap_or_default();
                                    *screenshot_data.write_unchecked() = Some(base64_string);
                                    *screenshot_bytes_sig.write_unchecked() = Some(bytes);
                                    *screenshot_status.write_unchecked() = format!(
                                        "🤖 Automation screenshot #{} ({}ms)",
                                        counter_val, duration_ms
                                    );
                                });
                            }
                            Ok(Err(e)) => {
                                *screenshot_status.write_unchecked() =
                                    format!("❌ Screenshot failed: {}", e);
                            }
                            Err(_) => {
                                *screenshot_status.write_unchecked() =
                                    "❌ Screenshot timeout (10s)".to_string();
                            }
                        }
                    });
                }
            }
            TimedEventType::Tap { x, y } => {
                if let Some(client) = &self.adb_client {
                    debug_print!(
                        self.debug_enabled,
                        "🎯 Queuing tap: {} at ({},{})",
                        event_id,
                        x,
                        y
                    );
                    let result = {
                        let client_guard = client.lock().await;
                        client_guard.tap(*x, *y).await
                    }; // Lock released here

                    match result {
                        Ok(()) => {
                            debug_print!(self.debug_enabled, "✅ {} queued", event_id);
                        }
                        Err(e) => {
                            let error_str = e.to_string();
                            println!("❌ {} queue failed: {}", event_id, error_str);

                            // Propagate disconnect errors to GUI
                            if is_disconnect_error(&error_str) {
                                debug_print!(
                                    self.debug_enabled,
                                    "🔌 Device disconnect detected during tap '{}': {}",
                                    event_id,
                                    error_str
                                );
                                self.device_disconnected = true;
                                self.last_reconnect_attempt = None;
                                *self.device_info.write_unchecked() = None;
                                *self.screenshot_data.write_unchecked() = None;
                                *self.screenshot_bytes.write_unchecked() = None;
                                *self.screenshot_status.write_unchecked() = format!(
                                    "🔌 USB DISCONNECTED: {} (during tap) - Please reconnect",
                                    error_str
                                );
                                *self.status.write_unchecked() =
                                    "🔌 Device Disconnected - Paused".to_string();
                            }
                        }
                    }
                } else {
                    return Err("ADB client not available".to_string());
                }
            }
            TimedEventType::CountdownUpdate => {
                self.send_timed_events_list().await;
                self.send_timed_tap_countdowns().await;

                // Log tap event status
                for (id, event) in &self.timed_events {
                    if id.contains("tap")
                        && let Some(last) = event.last_executed
                    {
                        let elapsed = last.elapsed();
                        let remaining = if elapsed < event.interval {
                            event.interval - elapsed
                        } else {
                            Duration::from_secs(0)
                        };
                        debug_print!(
                            self.debug_enabled,
                            "⏰ {}: {}s remaining",
                            id,
                            remaining.as_secs()
                        );
                    }
                }
            }
        }

        // Mark the event as executed
        if let Some(event) = self.timed_events.get_mut(event_id) {
            event.mark_executed();
        }

        // Event executed - timing updates will be sent via countdown_update event

        Ok(())
    }

    /// Update detector configuration
    pub fn update_match_config(&mut self, config: MatchConfig) {
        let threshold = config.confidence_threshold;
        let multiscale = config.enable_multiscale;
        self.game_detector.update_config(config);
        debug_print!(
            self.debug_enabled,
            "🔧 Match config updated: threshold={:.2}, multiscale={}",
            threshold,
            multiscale
        );
    }

    /// Reload templates
    pub async fn rescan_templates(&mut self) -> Result<(), String> {
        match self.game_detector.reload_templates(".") {
            Ok(count) => {
                debug_print!(self.debug_enabled, "🔄 Reloaded {} templates", count);
                // Templates reloaded - no GUI notification needed (templates are internal)
                Ok(())
            }
            Err(e) => {
                debug_print!(self.debug_enabled, "❌ Template reload failed: {}", e);
                Err(e)
            }
        }
    }

    /// Get current match configuration
    pub fn get_match_config(&self) -> &MatchConfig {
        self.game_detector.get_config()
    }

    /// Manual test of image recognition (for debugging)
    pub async fn test_image_recognition(&mut self) -> Result<(), String> {
        if let Some(screenshot_bytes) = self.latest_screenshot.clone() {
            debug_print!(
                self.debug_enabled,
                "🧪 Testing image recognition with current screenshot..."
            );
            match self.analyze_and_act(&screenshot_bytes).await {
                Ok(action_taken) => {
                    if action_taken {
                        debug_print!(
                            self.debug_enabled,
                            "✅ Test completed - action would be taken"
                        );
                    } else {
                        debug_print!(self.debug_enabled, "ℹ️ Test completed - no action needed");
                    }
                    Ok(())
                }
                Err(e) => {
                    debug_print!(self.debug_enabled, "❌ Test failed: {}", e);
                    Err(e)
                }
            }
        } else {
            let error = "No screenshot available for testing".to_string();
            debug_print!(self.debug_enabled, "⚠️ {}", error);
            Err(error)
        }
    }

    /// Analyze the current screenshot for patterns and perform actions if found
    async fn analyze_and_act(&mut self, screenshot_bytes: &[u8]) -> Result<bool, String> {
        debug_print!(self.debug_enabled, "🔍 Starting game state analysis...");

        // Move image analysis to background thread to prevent blocking the GUI
        let screenshot_data = screenshot_bytes.to_vec();
        let detector_config = self.game_detector.get_config().clone();
        let (screen_width, screen_height) = self.game_detector.get_screen_dimensions();

        debug_print!(
            self.debug_enabled,
            "🔄 Running image analysis in background thread..."
        );

        let detection_result = tokio::task::spawn_blocking(move || {
            // Create a temporary detector for this analysis
            let mut temp_detector =
                GameStateDetector::new(screen_width, screen_height, detector_config);

            // Load templates (this is also potentially blocking)
            if let Err(e) = temp_detector.load_templates(".") {
                return Err(format!("Failed to load templates: {}", e));
            }

            // Perform the analysis
            temp_detector.analyze_screenshot(&screenshot_data)
        })
        .await
        .map_err(|e| format!("Background analysis task failed: {}", e))??;

        debug_print!(
            self.debug_enabled,
            "🎯 Analysis complete: {} matches found (confidence: {:.3}, time: {}ms)",
            detection_result.matches.len(),
            detection_result.confidence_score,
            detection_result.processing_time_ms
        );

        // Act on the best match if available
        if let Some(best_match) = detection_result.best_match() {
            let (tap_x, tap_y) = best_match.get_tap_coordinates();

            debug_print!(
                self.debug_enabled,
                "🎯 Best match: '{}' at ({},{}) with {:.3} confidence",
                best_match.template.name,
                best_match.x,
                best_match.y,
                best_match.confidence
            );

            // Perform the tap action
            if let Some(client) = &self.adb_client {
                let client_guard = client.lock().await;

                match client_guard.tap(tap_x, tap_y).await {
                    Ok(()) => {
                        debug_print!(
                            self.debug_enabled,
                            "✅ Tapped '{}' at ({}, {})",
                            best_match.template.name,
                            tap_x,
                            tap_y
                        );

                        // Update game state based on detection result
                        if let Some(suggested_state) = detection_result.suggested_state {
                            // Don't change state here to avoid recursive state changes
                            debug_print!(
                                self.debug_enabled,
                                "💡 Suggested next state: {:?}",
                                suggested_state
                            );
                        }

                        Ok(true)
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to tap at ({}, {}): {}", tap_x, tap_y, e);

                        // Check if this is a disconnect error
                        if is_disconnect_error(&error_msg) {
                            debug_print!(
                                self.debug_enabled,
                                "🔌 Device disconnect detected during image recognition tap: {}",
                                error_msg
                            );
                            self.device_disconnected = true;
                            self.last_reconnect_attempt = None;
                            *self.device_info.write_unchecked() = None;
                            *self.screenshot_data.write_unchecked() = None;
                            *self.screenshot_bytes.write_unchecked() = None;
                            *self.screenshot_status.write_unchecked() =
                                format!("🔌 USB DISCONNECTED: {} - Please reconnect", error_msg);
                            *self.status.write_unchecked() =
                                "🔌 Device Disconnected - Paused".to_string();
                        }

                        Err(error_msg)
                    }
                }
            } else {
                Err("ADB client not available for tap action".to_string())
            }
        } else {
            debug_print!(self.debug_enabled, "👀 No actionable matches found");
            Ok(false)
        }
    }

    /// Send timed tap countdown updates to GUI
    async fn send_timed_tap_countdowns(&self) {
        // Find the next tap to fire (excluding system events)
        if let Some((next_tap_id, seconds_remaining)) = self.get_next_tap_info() {
            // Update countdown signal
            *self.timed_tap_countdown.write_unchecked() = Some((next_tap_id, seconds_remaining));
        }
    }

    /// Send current events list to GUI for display
    async fn send_timed_events_list(&self) {
        let events: Vec<crate::game_automation::types::TimedEvent> =
            self.timed_events.values().cloned().collect();
        *self.timed_events_list.write_unchecked() = events;
    }

    /// Get information about the next timed tap to fire (legacy compatibility)
    fn get_next_tap_info(&self) -> Option<(String, u64)> {
        let mut next_tap: Option<(String, u64)> = None;

        for (id, event) in &self.timed_events {
            // Only consider tap events for countdown display
            if !matches!(event.event_type, TimedEventType::Tap { .. }) {
                continue;
            }

            if !event.enabled {
                continue;
            }

            if let Some(time_until_next) = event.time_until_next() {
                let seconds_remaining = time_until_next.as_secs();

                match &next_tap {
                    None => {
                        next_tap = Some((id.clone(), seconds_remaining));
                    }
                    Some((_, current_min_seconds)) => {
                        if seconds_remaining < *current_min_seconds {
                            next_tap = Some((id.clone(), seconds_remaining));
                        }
                    }
                }
            }
        }

        next_tap
    }

    /// Check if it's time to attempt reconnection and send countdown updates
    async fn check_reconnection(&mut self) {
        // Use exponential backoff: 2s, 4s, 8s, 16s, 30s (max)
        let backoff_secs = match self.last_reconnect_attempt {
            None => 0, // First attempt immediately
            Some(last_attempt) => {
                let elapsed = std::time::Instant::now()
                    .duration_since(last_attempt)
                    .as_secs();
                
                // Count how many attempts we've made
                let attempt_count = (elapsed / 10).min(4); // Cap at 4 attempts tracked
                let backoff = 2u64.pow(attempt_count as u32);
                backoff.min(30) // Cap at 30 seconds between attempts
            }
        };

        let now = std::time::Instant::now();
        let should_attempt = match self.last_reconnect_attempt {
            None => true, // First attempt
            Some(last_attempt) => {
                let elapsed = now.duration_since(last_attempt).as_secs();
                elapsed >= backoff_secs
            }
        };

        if should_attempt {
            // Attempt reconnection
            println!(
                "🔄 Attempting device reconnection (elapsed: {:?})...",
                self.last_reconnect_attempt
                    .map(|t| now.duration_since(t))
                    .unwrap_or_default()
            );
            self.last_reconnect_attempt = Some(now);

            if let Ok(()) = self.attempt_reconnection().await {
                // Successful reconnection
                return;
            }
        }

        // Send countdown update to GUI
        if let Some(last_attempt) = self.last_reconnect_attempt {
            let elapsed = now.duration_since(last_attempt).as_secs();
            let backoff_secs = match self.last_reconnect_attempt {
                None => 0,
                Some(last_attempt) => {
                    let elapsed = std::time::Instant::now()
                        .duration_since(last_attempt)
                        .as_secs();
                    let attempt_count = (elapsed / 10).min(4);
                    let backoff = 2u64.pow(attempt_count as u32);
                    backoff.min(30)
                }
            };
            let remaining = backoff_secs.saturating_sub(elapsed);

            if remaining > 0 {
                *self.screenshot_status.write_unchecked() =
                    format!("🔌 Device disconnected - Next retry in {}s...", remaining);
            } else {
                *self.screenshot_status.write_unchecked() =
                    "🔌 Device disconnected - Attempting reconnection...".to_string();
            }
        }
    }

    /// Attempt to reconnect to the device
    async fn attempt_reconnection(&mut self) -> Result<(), String> {
        println!("🔄 Attempting to reconnect to device...");

        // CRITICAL: Clean up the old connection before creating a new one
        // This ensures the old USB processor task is stopped and device is released
        if let Some(old_client_arc) = self.adb_client.take() {
            println!("🔧 Shutting down old USB connection...");
            // Try to gracefully shutdown the old client
            match Arc::try_unwrap(old_client_arc) {
                Ok(mutex) => {
                    // Get the UsbAdb from the Mutex
                    let mut old_client = mutex.into_inner();
                    match old_client.shutdown().await {
                        Ok(_) => {
                            println!("✅ Old connection shut down cleanly");
                        }
                        Err(e) => {
                            println!("⚠️ Old connection shutdown warning: {}", e);
                        }
                    }
                }
                Err(arc) => {
                    // Arc still has other references, just drop and let them clean up
                    println!("⚠️ Old connection has other references, forcing drop...");
                    drop(arc);
                }
            }
            // Give cleanup time
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        match AdbBackend::connect_first().await {
            Ok(client) => {
                let (screen_width, screen_height) = client.screen_dimensions();
                println!("✅ Device reconnected! ({}x{})", screen_width, screen_height);

                // Update detector with screen dimensions
                let mut config = create_default_config();
                config.debug_enabled = self.debug_enabled;
                self.game_detector = GameStateDetector::new(screen_width, screen_height, config);

                // Store the new client
                self.adb_client = Some(Arc::new(Mutex::new(client)));

                // Restart touch monitoring
                if let Some(client_arc) = &self.adb_client {
                    let client_guard = client_arc.lock().await;
                    if let Err(e) = client_guard.start_touch_monitoring().await {
                        println!("⚠️ Failed to start touch monitoring after reconnect: {}", e);
                    } else {
                        println!("👆 Touch monitoring restarted");
                    }
                }

                // Mark as reconnected
                self.device_disconnected = false;
                self.last_reconnect_attempt = None;

                // Auto-resume automation if it was running (same as initial startup)
                if self.is_running && self.state == GameState::Paused {
                    self.change_state(GameState::Running).await;
                    println!("▶️ Auto-resuming automation after reconnection");
                }

                // Update reconnection signals
                if let Some(client_arc) = &self.adb_client {
                    let client_guard = client_arc.lock().await;
                    let (sx, sy) = client_guard.screen_dimensions();
                    *self.device_info.write_unchecked() = Some((
                        client_guard.device_name().to_string(),
                        client_guard.transport_id(),
                        sx,
                        sy,
                    ));
                }
                *self.screenshot_status.write_unchecked() =
                    "✅ Reconnected! Automation ready.".to_string();
                *self.status.write_unchecked() = "✅ Device Reconnected - Resuming".to_string();

                println!("✅ Device reconnected successfully - automation auto-resumed");

                Ok(())
            }
            Err(e) => {
                println!("❌ Reconnection failed: {}", e);
                Err(e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timed_event_interval_tracking() {
        // Test that TimedEvent correctly tracks intervals
        let mut event = TimedEvent::new_tap_seconds("test_tap".to_string(), 100, 100, 2);

        // Initially should be ready (time=0)
        assert!(event.is_ready(false));
        // When ready, returns Some(Duration::ZERO)
        let time_until = event.time_until_next().unwrap();
        assert_eq!(time_until.as_secs(), 0, "Should be ready immediately");

        // Mark as executed
        event.mark_executed();

        // Should not be ready immediately after
        assert!(!event.is_ready(false));

        // Time until ready should be close to 2 seconds (allow small variance)
        let remaining = event.time_until_next().unwrap();
        let remaining_secs = remaining.as_secs();
        assert!(
            remaining_secs <= 2,
            "Expected ~2s remaining, got {}s",
            remaining_secs
        );
        assert!(
            remaining_secs > 0,
            "Expected ~2s remaining, got {}s",
            remaining_secs
        );
    }

    #[test]
    fn test_multiple_timed_events() {
        // Test that multiple events can be tracked independently
        let mut events = HashMap::new();

        events.insert(
            "event1".to_string(),
            TimedEvent::new_tap_seconds("event1".to_string(), 100, 100, 5),
        );
        events.insert(
            "event2".to_string(),
            TimedEvent::new_tap_seconds("event2".to_string(), 200, 200, 10),
        );

        // Both should start ready
        assert!(events.get("event1").unwrap().is_ready(false));
        assert!(events.get("event2").unwrap().is_ready(false));

        // Execute first event
        events.get_mut("event1").unwrap().mark_executed();

        // First should not be ready, second still ready
        assert!(!events.get("event1").unwrap().is_ready(false));
        assert!(events.get("event2").unwrap().is_ready(false));

        // Check intervals are different
        let remaining1 = events.get("event1").unwrap().time_until_next().unwrap();
        assert!(remaining1.as_secs() <= 5, "Event1 interval incorrect");

        // Execute second event
        events.get_mut("event2").unwrap().mark_executed();
        let remaining2 = events.get("event2").unwrap().time_until_next().unwrap();
        assert!(remaining2.as_secs() <= 10, "Event2 interval incorrect");
        assert!(
            remaining2 > remaining1,
            "Event2 should have longer interval"
        );
    }

    #[tokio::test]
    async fn test_lock_scope_prevents_deadlock() {
        // This test verifies that locks are properly scoped and released
        use tokio::sync::Mutex;

        let counter = Arc::new(Mutex::new(0u32));
        let counter_clone = counter.clone();

        // Simulate the pattern used in execute_timed_event
        let task = tokio::spawn(async move {
            for _i in 0..5 {
                // Scoped lock (like in the fix)
                {
                    let mut guard = counter_clone.lock().await;
                    *guard += 1;
                } // Lock released here

                // Small delay between operations
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        // Wait for task to complete with timeout
        let result = tokio::time::timeout(Duration::from_secs(1), task).await;

        // Should not timeout (would timeout if locks were held)
        assert!(result.is_ok(), "Task timed out - potential deadlock");

        // Check final value
        let final_value = *counter.lock().await;
        assert_eq!(final_value, 5, "Expected 5 increments, got {}", final_value);
    }
}
