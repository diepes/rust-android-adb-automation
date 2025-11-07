// Finite State Machine implementation for game automation
use super::match_image::{GameStateDetector, MatchConfig, create_default_config};
use super::types::{AutomationCommand, AutomationEvent, GameState, TimedTap};
use crate::adb::AdbBackend;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, Instant, sleep};

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
    screenshot_interval: Duration,
    adb_client: Option<Arc<Mutex<AdbBackend>>>,
    command_rx: mpsc::Receiver<AutomationCommand>,
    event_tx: mpsc::Sender<AutomationEvent>,
    is_running: bool,
    should_exit: bool,
    debug_enabled: bool,
    // New image matching system
    latest_screenshot: Option<Vec<u8>>, // Raw PNG bytes
    game_detector: GameStateDetector,
    // Timed taps system
    timed_taps: HashMap<String, TimedTap>,
    last_countdown_update: Instant,
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

        let mut timed_taps = HashMap::new();
        
        // Add default timed tap at 150,1270 every 5 minutes
        let default_tap = TimedTap::new(
            "claim_5d_tap".to_string(),
            150,
            1270,
            5, // 5 minutes
        );
        
        if debug_enabled {
            println!("🕒 Initialized default timed tap: {} at ({},{}) every {}min", 
                default_tap.id, default_tap.x, default_tap.y, default_tap.interval.as_secs() / 60);
        }
        
        timed_taps.insert("default_tap".to_string(), default_tap);

        Self {
            state: GameState::Idle,
            screenshot_interval: Duration::from_secs(30),
            adb_client: None,
            command_rx,
            event_tx,
            is_running: false,
            should_exit: false,
            debug_enabled,
            latest_screenshot: None,
            game_detector,
            timed_taps,
            last_countdown_update: Instant::now(),
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
        if let Some(client) = &self.adb_client {
            let client_guard = client.lock().await;
            match client_guard.screen_capture_bytes().await {
                Ok(bytes) => {
                    debug_print!(
                        self.debug_enabled,
                        "📸 Game automation captured screenshot ({} bytes)",
                        bytes.len()
                    );

                    // Store the latest screenshot for image recognition
                    self.latest_screenshot = Some(bytes.clone());

                    let _ = self
                        .event_tx
                        .send(AutomationEvent::ScreenshotReady(bytes.clone()))
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
                    self.change_state(GameState::WaitingForScreenshot).await;
                    debug_print!(
                        self.debug_enabled,
                        "🚀 Game automation started (interval: {}s). Timed taps: {} configured",
                        self.screenshot_interval.as_secs(),
                        self.timed_taps.len()
                    );
                    
                    // List timed taps on start for debugging
                    if self.debug_enabled {
                        for (id, tap) in &self.timed_taps {
                            let next_in = tap.time_until_next()
                                .map(|d| format!("{:.1}min", d.as_secs() as f32 / 60.0))
                                .unwrap_or_else(|| "disabled".to_string());
                            debug_print!(
                                self.debug_enabled,
                                "  🕒 Timed tap '{}': ({},{}) every {}min, next in {}",
                                id, tap.x, tap.y, tap.interval.as_secs() / 60, next_in
                            );
                        }
                    }
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
                    self.change_state(GameState::WaitingForScreenshot).await;
                    debug_print!(self.debug_enabled, "▶️ Game automation resumed");
                }
            }
            AutomationCommand::Stop => {
                self.is_running = false;
                self.change_state(GameState::Idle).await;
                debug_print!(self.debug_enabled, "⏹️ Game automation stopped");
            }
            AutomationCommand::TakeScreenshot => {
                if self.is_running && self.state != GameState::Paused {
                    self.change_state(GameState::WaitingForScreenshot).await;
                    let _ = self.take_screenshot().await;
                }
            }
            AutomationCommand::UpdateInterval(seconds) => {
                self.screenshot_interval = Duration::from_secs(seconds);
                let _ = self
                    .event_tx
                    .send(AutomationEvent::IntervalUpdate(seconds))
                    .await;
                debug_print!(
                    self.debug_enabled,
                    "⏱️ Screenshot interval updated to {}s",
                    seconds
                );
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
            AutomationCommand::AddTimedTap(timed_tap) => {
                debug_print!(
                    self.debug_enabled,
                    "➕ Adding timed tap '{}' at ({},{}) every {}min",
                    timed_tap.id,
                    timed_tap.x,
                    timed_tap.y,
                    timed_tap.interval.as_secs() / 60
                );
                self.timed_taps.insert(timed_tap.id.clone(), timed_tap);
            }
            AutomationCommand::RemoveTimedTap(id) => {
                if self.timed_taps.remove(&id).is_some() {
                    debug_print!(self.debug_enabled, "➖ Removed timed tap '{}'", id);
                } else {
                    debug_print!(self.debug_enabled, "⚠️ Timed tap '{}' not found for removal", id);
                }
            }
            AutomationCommand::EnableTimedTap(id) => {
                if let Some(timed_tap) = self.timed_taps.get_mut(&id) {
                    timed_tap.enabled = true;
                    debug_print!(self.debug_enabled, "✅ Enabled timed tap '{}'", id);
                } else {
                    debug_print!(self.debug_enabled, "⚠️ Timed tap '{}' not found for enabling", id);
                }
            }
            AutomationCommand::DisableTimedTap(id) => {
                if let Some(timed_tap) = self.timed_taps.get_mut(&id) {
                    timed_tap.enabled = false;
                    debug_print!(self.debug_enabled, "❌ Disabled timed tap '{}'", id);
                } else {
                    debug_print!(self.debug_enabled, "⚠️ Timed tap '{}' not found for disabling", id);
                }
            }
            AutomationCommand::ListTimedTaps => {
                let taps: Vec<TimedTap> = self.timed_taps.values().cloned().collect();
                debug_print!(self.debug_enabled, "📋 Listing {} timed taps", taps.len());
                for tap in &taps {
                    let status = if tap.enabled { "enabled" } else { "disabled" };
                    let next_time = match tap.time_until_next() {
                        Some(duration) => format!("{:.1}min", duration.as_secs() as f32 / 60.0),
                        None => "disabled".to_string(),
                    };
                    debug_print!(
                        self.debug_enabled,
                        "  - {}: ({},{}) every {}min, {}, next in {}",
                        tap.id,
                        tap.x,
                        tap.y,
                        tap.interval.as_secs() / 60,
                        status,
                        next_time
                    );
                }
                let _ = self.event_tx.send(AutomationEvent::TimedTapsListed(taps)).await;
            }
            AutomationCommand::Shutdown => {
                self.should_exit = true;
                self.is_running = false;
                self.change_state(GameState::Idle).await;
                println!("🛑 Game automation shutting down");
            }
        }
    }

    pub async fn run(&mut self) {
        debug_print!(self.debug_enabled, "🎮 Game automation FSM loop started");
        let mut last_screenshot = Instant::now();

        loop {
            // Check for commands (non-blocking)
            if let Ok(command) = self.command_rx.try_recv() {
                self.process_command(command).await;
            }

            // Check and execute ready timed taps (only when automation is running and not paused)
            if self.is_running && self.state != GameState::Paused {
                self.check_and_execute_timed_taps().await;
                
                // Send countdown updates every 5 seconds
                if self.last_countdown_update.elapsed() >= Duration::from_secs(5) {
                    self.send_timed_tap_countdowns().await;
                    self.last_countdown_update = Instant::now();
                }
            }

            // FSM logic based on current state
            match self.state {
                GameState::Idle => {
                    // Wait for start command
                    sleep(Duration::from_millis(100)).await;
                }
                GameState::Paused => {
                    // Wait for resume or stop command
                    sleep(Duration::from_millis(100)).await;
                }
                GameState::WaitingForScreenshot => {
                    if self.is_running && last_screenshot.elapsed() >= self.screenshot_interval {
                        self.change_state(GameState::Analyzing).await;
                        match self.take_screenshot().await {
                            Ok(_) => {
                                last_screenshot = Instant::now();
                                // Transition to Acting state for potential game actions
                                self.change_state(GameState::Acting).await;
                            }
                            Err(e) => {
                                eprintln!("❌ Screenshot error: {}", e);
                                // Stay in waiting state, will retry on next interval
                            }
                        }
                    } else {
                        // Check every 100ms but only screenshot on interval
                        sleep(Duration::from_millis(100)).await;
                    }
                }
                GameState::Analyzing => {
                    // Placeholder for image analysis logic
                    // For now, just transition to Acting
                    self.change_state(GameState::Acting).await;
                }
                GameState::Acting => {
                    debug_print!(
                        self.debug_enabled,
                        "🎮 Entering Acting state - performing image recognition..."
                    );

                    // Perform image recognition and actions
                    if let Some(screenshot_bytes) = &self.latest_screenshot {
                        debug_print!(
                            self.debug_enabled,
                            "📸 Screenshot available ({} bytes), analyzing...",
                            screenshot_bytes.len()
                        );

                        match self.analyze_and_act(screenshot_bytes).await {
                            Ok(action_taken) => {
                                if action_taken {
                                    debug_print!(
                                        self.debug_enabled,
                                        "🎯 Game action executed successfully!"
                                    );
                                    // Wait a bit after taking action before next screenshot
                                    sleep(Duration::from_millis(1000)).await;
                                } else {
                                    debug_print!(
                                        self.debug_enabled,
                                        "👀 No matching patterns found, continuing scan..."
                                    );
                                    // No action needed, wait shorter time
                                    sleep(Duration::from_millis(500)).await;
                                }
                            }
                            Err(e) => {
                                debug_print!(self.debug_enabled, "❌ Image analysis error: {}", e);
                                sleep(Duration::from_millis(500)).await;
                            }
                        }
                    } else {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ No screenshot available for analysis"
                        );
                        sleep(Duration::from_millis(500)).await;
                    }

                    // Return to waiting for next screenshot
                    self.change_state(GameState::WaitingForScreenshot).await;
                }
            }

            // Break the loop if shutdown was requested
            if self.should_exit {
                break;
            }
        }

        debug_print!(self.debug_enabled, "🎮 Game automation FSM loop ended");
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
        
        debug_print!(self.debug_enabled, "🔄 Running image analysis in background thread...");
        
        let detection_result = tokio::task::spawn_blocking(move || {
            // Create a temporary detector for this analysis
            let mut temp_detector = GameStateDetector::new(screen_width, screen_height, detector_config);
            
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

    /// Check and execute any timed taps that are ready
    async fn check_and_execute_timed_taps(&mut self) {
        let mut taps_to_execute = Vec::new();
        
        // Collect taps that are ready (avoid borrowing issues)
        for (id, tap) in &self.timed_taps {
            if tap.is_ready() {
                taps_to_execute.push((id.clone(), tap.x, tap.y));
            }
        }
        
        // Execute ready taps
        for (tap_id, x, y) in taps_to_execute {
            if let Err(e) = self.execute_timed_tap(&tap_id, x, y).await {
                debug_print!(
                    self.debug_enabled,
                    "❌ Timed tap '{}' failed: {}",
                    tap_id,
                    e
                );
                let _ = self
                    .event_tx
                    .send(AutomationEvent::Error(format!(
                        "Timed tap '{}' failed: {}",
                        tap_id, e
                    )))
                    .await;
            }
        }
    }

    /// Execute a specific timed tap
    async fn execute_timed_tap(&mut self, tap_id: &str, x: u32, y: u32) -> Result<(), String> {
        debug_print!(
            self.debug_enabled,
            "🕒 Executing timed tap '{}' at ({},{})",
            tap_id,
            x,
            y
        );

        if let Some(client) = &self.adb_client {
            let client_guard = client.lock().await;
            
            match client_guard.tap(x, y).await {
                Ok(()) => {
                    // Mark the tap as executed
                    if let Some(tap) = self.timed_taps.get_mut(tap_id) {
                        tap.mark_executed();
                        
                        // Calculate next execution time for logging
                        let next_in_mins = tap.interval.as_secs() as f32 / 60.0;
                        
                        debug_print!(
                            self.debug_enabled,
                            "✅ Timed tap '{}' executed at ({},{}). Next in {:.1}min",
                            tap_id,
                            x,
                            y,
                            next_in_mins
                        );
                        
                        // Send event notification
                        let _ = self
                            .event_tx
                            .send(AutomationEvent::TimedTapExecuted(
                                tap_id.to_string(),
                                x,
                                y,
                            ))
                            .await;
                    }
                    
                    Ok(())
                }
                Err(e) => Err(format!("ADB tap failed: {}", e)),
            }
        } else {
            Err("ADB client not available".to_string())
        }
    }

    /// Send timed tap countdown updates to GUI
    async fn send_timed_tap_countdowns(&self) {
        for (id, tap) in &self.timed_taps {
            if tap.enabled {
                if let Some(time_until_next) = tap.time_until_next() {
                    let seconds_remaining = time_until_next.as_secs();
                    let _ = self
                        .event_tx
                        .send(AutomationEvent::TimedTapCountdown(
                            id.clone(),
                            seconds_remaining,
                        ))
                        .await;
                }
            }
        }
    }
}
