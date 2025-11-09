// Finite State Machine implementation for game automation - Event Driven Architecture
use super::match_image::{GameStateDetector, MatchConfig, create_default_config};
use super::types::{AutomationCommand, AutomationEvent, GameState, TimedEvent, TimedEventType};
use crate::adb::AdbBackend;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, timeout};

// Macro for debug output
macro_rules! debug_print {
    ($debug_enabled:expr, $($arg:tt)*) => {
        if $debug_enabled {
            println!($($arg)*);
        }
    };
}

pub struct GameAutomation {
    state: GameState,
    adb_client: Option<Arc<Mutex<AdbBackend>>>,
    command_rx: mpsc::Receiver<AutomationCommand>,
    event_tx: mpsc::Sender<AutomationEvent>,
    is_running: bool,
    should_exit: bool,
    debug_enabled: bool,
    // New image matching system
    latest_screenshot: Option<Vec<u8>>, // Raw PNG bytes
    game_detector: GameStateDetector,
    // Unified timed events system
    timed_events: HashMap<String, TimedEvent>,
}

impl GameAutomation {
    pub fn new(
        command_rx: mpsc::Receiver<AutomationCommand>,
        event_tx: mpsc::Sender<AutomationEvent>,
        debug_enabled: bool,
    ) -> Self {
        // Create default detector (will be updated with screen dimensions later)
        let config = create_default_config();
        let game_detector = GameStateDetector::new(1080, 2400, config); // Default dimensions

        let mut timed_events = HashMap::new();

        // Create core system events
        timed_events.insert(
            "screenshot".to_string(),
            TimedEvent::new_screenshot(600), // Default 30-second interval
        );
        timed_events.insert(
            "countdown_update".to_string(),
            TimedEvent::new_countdown_update(1),
        );

        // Define all timed taps in a list for easier management
        let tap_definitions = vec![
            ("claim_5d_tap", 110, 1300, 2), // 5 minutes
            ("restart_tap", 110, 1600, 5),  // 9 minutes
                                            // Add more taps here as needed
        ];

        // Create and insert all timed tap events
        for (id, x, y, interval_minutes) in tap_definitions {
            let event = TimedEvent::new_tap(id.to_string(), x, y, interval_minutes);
            timed_events.insert(id.to_string(), event);
        }

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
            event_tx,
            is_running: false,
            should_exit: false,
            debug_enabled,
            latest_screenshot: None,
            game_detector,
            timed_events,
        }
    }

    pub async fn initialize_adb(&mut self, use_rust_impl: bool) -> Result<(), String> {
        match AdbBackend::connect_first(use_rust_impl).await {
            Ok(client) => {
                let (screen_width, screen_height) = client.screen_dimensions();

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
                        // Create template names for GUI notification
                        let template_names: Vec<String> =
                            (0..count).map(|i| format!("template_{}", i)).collect();
                        let _ = self
                            .event_tx
                            .send(AutomationEvent::TemplatesUpdated(template_names))
                            .await;
                    }
                    Err(e) => {
                        debug_print!(self.debug_enabled, "⚠️ Template loading warning: {}", e);
                    }
                }

                self.adb_client = Some(Arc::new(Mutex::new(client)));
                debug_print!(
                    self.debug_enabled,
                    "🤖 Game automation initialized ({}x{})",
                    screen_width,
                    screen_height
                );
                Ok(())
            }
            Err(e) => {
                let error = format!("Failed to initialize ADB for automation: {}", e);
                let _ = self
                    .event_tx
                    .send(AutomationEvent::Error(error.clone()))
                    .await;
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
            let _ = self
                .event_tx
                .send(AutomationEvent::StateChanged(new_state))
                .await;
        }
    }

    async fn take_screenshot(&mut self) -> Result<Vec<u8>, String> {
        let start_time = std::time::Instant::now();

        if let Some(client) = &self.adb_client {
            let client_guard = client.lock().await;
            match client_guard.screen_capture_bytes().await {
                Ok(bytes) => {
                    let duration_ms = start_time.elapsed().as_millis();

                    debug_print!(
                        self.debug_enabled,
                        "📸 Game automation captured screenshot ({} bytes) in {}ms",
                        bytes.len(),
                        duration_ms
                    );

                    // Store the latest screenshot for image recognition
                    self.latest_screenshot = Some(bytes.clone());

                    // Send screenshot ready event with timing information
                    let _ = self
                        .event_tx
                        .send(AutomationEvent::ScreenshotTaken(
                            bytes.clone(),
                            duration_ms as u64,
                        ))
                        .await;
                    Ok(bytes)
                }
                Err(e) => {
                    let error = format!("Screenshot failed: {}", e);
                    let _ = self
                        .event_tx
                        .send(AutomationEvent::Error(error.clone()))
                        .await;
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
                    self.is_running = true;
                    self.change_state(GameState::Running).await;
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
                self.change_state(GameState::Idle).await;
                debug_print!(self.debug_enabled, "⏹️ Game automation stopped");
            }
            AutomationCommand::TakeScreenshot => {
                if self.is_running && self.state != GameState::Paused {
                    let _ = self.take_screenshot().await;
                }
            }
            AutomationCommand::UpdateInterval(seconds) => {
                // Update screenshot event interval
                if let Some(screenshot_event) = self.timed_events.get_mut("screenshot") {
                    screenshot_event.interval = Duration::from_secs(seconds);
                    debug_print!(
                        self.debug_enabled,
                        "⏱️ Screenshot interval updated to {}s",
                        seconds
                    );
                }
                let _ = self
                    .event_tx
                    .send(AutomationEvent::IntervalUpdate(seconds))
                    .await;
            }
            AutomationCommand::TestImageRecognition => {
                debug_print!(
                    self.debug_enabled,
                    "🧪 Manual image recognition test requested"
                );
                if let Err(e) = self.test_image_recognition().await {
                    let _ = self.event_tx.send(AutomationEvent::Error(e)).await;
                }
            }
            AutomationCommand::RescanTemplates => {
                debug_print!(self.debug_enabled, "🔄 Template rescan requested");
                if let Err(e) = self.rescan_templates().await {
                    let _ = self.event_tx.send(AutomationEvent::Error(e)).await;
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
            AutomationCommand::TriggerTimedEvent(id) => {
                if let Some(event) = self.timed_events.get(&id) {
                    if event.enabled {
                        debug_print!(self.debug_enabled, "🔫 Triggering timed event '{}' immediately", id);
                        // Execute the event immediately
                        match event.event_type {
                            TimedEventType::Screenshot => {
                                let _ = self.take_screenshot().await;
                            }
                            TimedEventType::Tap { x, y } => {
                                if let Some(adb_client) = &self.adb_client {
                                    let client = adb_client.lock().await;
                                    if let Err(e) = client.tap(x, y).await {
                                        debug_print!(self.debug_enabled, "⚠️ Failed to execute tap ({}, {}): {}", x, y, e);
                                    } else {
                                        let _ = self.event_tx.send(AutomationEvent::TimedTapExecuted(id.clone(), x, y)).await;
                                    }
                                }
                            }
                            TimedEventType::CountdownUpdate => {
                                // Don't trigger countdown updates manually, they're system events
                                debug_print!(self.debug_enabled, "⚠️ Cannot manually trigger countdown update event");
                            }
                        }
                        // Mark as executed and send events list update
                        if let Some(event) = self.timed_events.get_mut(&id) {
                            event.mark_executed();
                        }
                        self.send_timed_events_list().await;
                        let _ = self.event_tx.send(AutomationEvent::TimedEventExecuted(id)).await;
                    } else {
                        debug_print!(self.debug_enabled, "⚠️ Cannot trigger disabled event '{}'", id);
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
                let _ = self
                    .event_tx
                    .send(AutomationEvent::TimedEventsListed(events))
                    .await;
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

        loop {
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
                    debug_print!(self.debug_enabled, "⏱️ Command timeout, processing events");
                }
            }

            // Process timed events if automation is running and not paused
            if self.is_running && self.state != GameState::Paused {
                self.process_timed_events().await;
            }

            // Break the loop if shutdown was requested
            if self.should_exit {
                break;
            }
        }

        debug_print!(self.debug_enabled, "🎮 Event-driven automation FSM ended");
    }

    /// Process all ready timed events
    async fn process_timed_events(&mut self) {
        let mut events_to_execute = Vec::new();

        // Collect ready events
        for (id, event) in &self.timed_events {
            if event.is_ready() {
                events_to_execute.push((id.clone(), event.event_type.clone()));
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
                let _ = self
                    .event_tx
                    .send(AutomationEvent::Error(format!(
                        "Timed event '{}' failed: {}",
                        event_id, e
                    )))
                    .await;
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
                match self.take_screenshot().await {
                    Ok(_) => {
                        // Trigger image analysis after screenshot
                        if let Some(screenshot_bytes) = &self.latest_screenshot {
                            let _ = self.analyze_and_act(screenshot_bytes).await;
                        }
                    }
                    Err(e) => return Err(format!("Screenshot failed: {}", e)),
                }
            }
            TimedEventType::Tap { x, y } => {
                if let Some(client) = &self.adb_client {
                    let client_guard = client.lock().await;
                    match client_guard.tap(*x, *y).await {
                        Ok(()) => {
                            debug_print!(
                                self.debug_enabled,
                                "✅ Timed tap '{}' executed at ({},{})",
                                event_id,
                                x,
                                y
                            );
                            let _ = self
                                .event_tx
                                .send(AutomationEvent::TimedTapExecuted(
                                    event_id.to_string(),
                                    *x,
                                    *y,
                                ))
                                .await;
                        }
                        Err(e) => return Err(format!("ADB tap failed: {}", e)),
                    }
                } else {
                    return Err("ADB client not available".to_string());
                }
            }
            TimedEventType::CountdownUpdate => {
                self.send_timed_events_list().await;
                self.send_timed_tap_countdowns().await;
            }
        }

        // Mark the event as executed
        if let Some(event) = self.timed_events.get_mut(event_id) {
            event.mark_executed();
        }

        let _ = self
            .event_tx
            .send(AutomationEvent::TimedEventExecuted(event_id.to_string()))
            .await;

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
                let template_paths: Vec<String> =
                    (0..count).map(|i| format!("template_{}", i)).collect();
                let _ = self
                    .event_tx
                    .send(AutomationEvent::TemplatesUpdated(template_paths))
                    .await;
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
    pub async fn test_image_recognition(&self) -> Result<(), String> {
        if let Some(screenshot_bytes) = &self.latest_screenshot {
            debug_print!(
                self.debug_enabled,
                "🧪 Testing image recognition with current screenshot..."
            );
            match self.analyze_and_act(screenshot_bytes).await {
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
    async fn analyze_and_act(&self, screenshot_bytes: &[u8]) -> Result<bool, String> {
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

                        return Ok(true);
                    }
                    Err(e) => {
                        return Err(format!("Failed to tap at ({}, {}): {}", tap_x, tap_y, e));
                    }
                }
            } else {
                return Err("ADB client not available for tap action".to_string());
            }
        } else {
            debug_print!(self.debug_enabled, "👀 No actionable matches found");
            return Ok(false);
        }
    }

    /// Send timed tap countdown updates to GUI
    async fn send_timed_tap_countdowns(&self) {
        // Find the next tap to fire (excluding system events)
        if let Some((next_tap_id, seconds_remaining)) = self.get_next_tap_info() {
            // Send the countdown for the next tap to fire
            let _ = self
                .event_tx
                .send(AutomationEvent::TimedTapCountdown(
                    next_tap_id,
                    seconds_remaining,
                ))
                .await;
        }
    }

    /// Send current events list to GUI for display
    async fn send_timed_events_list(&self) {
        let events: Vec<crate::game_automation::types::TimedEvent> = self.timed_events.values().cloned().collect();
        let _ = self
            .event_tx
            .send(AutomationEvent::TimedEventsListed(events))
            .await;
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
}
