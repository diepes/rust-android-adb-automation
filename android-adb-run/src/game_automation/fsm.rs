// Finite State Machine implementation for game automation - Event Driven Architecture
use super::config::load_or_create_timed_events;
use super::match_image::{GameStateDetector, MatchConfig, create_default_config};
use super::types::{
    AutomationCommand, DeviceInfo, GameState, MAX_TAP_INTERVAL_SECONDS, MIN_TAP_INTERVAL_SECONDS,
    TimedEvent, TimedEventType,
};
use crate::adb::{AdbBackend, AdbClient};
use crate::gui::hooks::device_loop::start_template_matching_phase;
use dioxus::prelude::{Signal, WritableExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, timeout};

mod commands;
mod reconnect;
mod run_loop;
mod scheduler;

// Helper function to detect if an error indicates device disconnection
// NOTE: This distinguishes between operational CLSE errors (need reconnect) and cleanup CLSE (harmless)
pub fn is_disconnect_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();

    // CRITICAL: Protocol desync errors (CLSE during operations) ARE disconnects
    // These errors contain specific phrases indicating the ADB protocol is out of sync
    // and the connection MUST be re-established
    if error_lower.contains("protocol desync")
        || error_lower.contains("reconnection needed")
        || error_lower.contains("connection needs to be re-established")
    {
        return true;
    }

    // CLSE errors during actual command execution (tap/screenshot) need reconnection
    // But standalone "CLSE" during cleanup (without "failed" or command context) is harmless
    let is_operational_clse = error_lower.contains("clse")
        && (error_lower.contains("failed")
            || error_lower.contains("command")
            || error_lower.contains("tap")
            || error_lower.contains("screencap")
            || error_lower.contains("input"));

    if is_operational_clse {
        return true;
    }

    // Harmless cleanup CLSE messages (no command context)
    let is_harmless_clse = error_lower.contains("clse")
        && !error_lower.contains("failed")
        && !error_lower.contains("command");

    if is_harmless_clse {
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
        || error_lower.contains("no write endpoint")   // USB write endpoint lost
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
    screenshot_status_history: Signal<Vec<(String, bool)>>,
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

        let timed_events = load_or_create_timed_events(debug_enabled);

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
            screenshot_status_history: signals.screenshot_status_history,
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
                    let counter_val = self.screenshot_counter.with_mut(|c| {
                        *c += 1;
                        *c
                    });
                    let screenshot_data_clone = self.screenshot_data;
                    let screenshot_bytes_clone = self.screenshot_bytes;
                    let screenshot_status_clone = self.screenshot_status;
                    let status_history_for_matching = self.screenshot_status_history;
                    let status_signal_for_matching = self.screenshot_status;
                    let matching_bytes = bytes.clone();

                    // Spawn base64 encoding in background to avoid blocking
                    dioxus::prelude::spawn(async move {
                        use crate::gui::util::base64_encode;
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

                    // Start template matching so Progress History updates for automation captures as well
                    start_template_matching_phase(
                        matching_bytes,
                        None,
                        counter_val as u32,
                        status_signal_for_matching,
                        status_history_for_matching,
                    );

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

    #[test]
    fn test_is_disconnect_error_clse_protocol_desync() {
        // Test that CLSE protocol desync errors during operations ARE detected as disconnects
        // This was the root cause of the GUI hung state bug

        // These should be detected as disconnect errors (need reconnection)
        let protocol_desync_errors = [
            "ADB protocol desync (CLSE error) - connection needs to be re-established: Command 'input tap' failed with protocol error: ADB request failed - wrong command CLSE",
            "ADB protocol desync (CLSE error) - connection needs to be re-established: Command 'screencap -p' failed with protocol error",
            "PROTOCOL DESYNC - reconnection needed",
            "❌ Tap failed (PROTOCOL DESYNC - reconnection needed): error at (350,628)",
            "Command 'input tap' failed with CLSE error",
            "screencap -p command failed: CLSE",
        ];

        for error in protocol_desync_errors {
            assert!(
                is_disconnect_error(error),
                "Should detect protocol desync as disconnect: {}",
                error
            );
        }

        // These should NOT be detected as disconnect errors (harmless cleanup)
        let harmless_clse_errors = [
            "CLSE",                 // Just CLSE without context
            "Received CLSE packet", // Protocol acknowledgment
        ];

        for error in harmless_clse_errors {
            assert!(
                !is_disconnect_error(error),
                "Should NOT detect harmless CLSE as disconnect: {}",
                error
            );
        }

        // Standard disconnect errors should still work
        let standard_disconnect_errors = [
            "device offline",
            "device not found",
            "no devices",
            "connection refused",
            "broken pipe",
            "connection reset",
            "no write endpoint",
        ];

        for error in standard_disconnect_errors {
            assert!(
                is_disconnect_error(error),
                "Should detect standard disconnect: {}",
                error
            );
        }
    }
}
