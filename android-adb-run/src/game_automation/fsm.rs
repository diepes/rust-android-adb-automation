// Finite State Machine implementation for game automation
use crate::adb::AdbBackend;
use super::types::{GameState, AutomationCommand, AutomationEvent};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration, Instant};

// Macro for debug output
macro_rules! debug_print {
    ($debug_enabled:expr, $($arg:tt)*) => {
        if $debug_enabled {
            println!($($arg)*);
        }
    };
}

// Configuration for image recognition
#[derive(Debug, Clone)]
pub struct ImageRecognitionConfig {
    template_path: String,
    confidence_threshold: f32,
    template_width: u32,
    template_height: u32,
}

impl Default for ImageRecognitionConfig {
    fn default() -> Self {
        Self {
            template_path: "img-[300,1682,50,50].png".to_string(),
            confidence_threshold: 0.8,
            template_width: 50,
            template_height: 50,
        }
    }
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
    // Image recognition
    latest_screenshot: Option<Vec<u8>>, // Raw PNG bytes
    image_config: ImageRecognitionConfig,
}

// Image recognition module
mod image_recognition {
    use imageproc::template_matching::{match_template, MatchTemplateMethod};
    
    pub struct MatchResult {
        pub found: bool,
        pub x: u32,
        pub y: u32,
        pub confidence: f32,
    }
    
    pub fn find_template_in_image(
        screenshot_bytes: &[u8], 
        template_path: &str,
        threshold: f32,
    ) -> Result<MatchResult, String> {
        // Load the screenshot from PNG bytes
        let screenshot = image::load_from_memory(screenshot_bytes)
            .map_err(|e| format!("Failed to load screenshot: {e}"))?;
        
        // Load the template image
        let template = image::open(template_path)
            .map_err(|e| format!("Failed to load template {}: {e}", template_path))?;
        
        // Convert to grayscale for template matching
        let screenshot_gray = screenshot.to_luma8();
        let template_gray = template.to_luma8();
        
        // Perform template matching using normalized cross correlation
        let result = match_template(&screenshot_gray, &template_gray, MatchTemplateMethod::CrossCorrelationNormalized);
        
        // Find the best match
        let mut max_score = 0.0f32;
        let mut best_x = 0u32;
        let mut best_y = 0u32;
        
        for (x, y, pixel) in result.enumerate_pixels() {
            let score = pixel[0] as f32 / 255.0; // Normalize to 0.0-1.0 range
            if score > max_score {
                max_score = score;
                best_x = x;
                best_y = y;
            }
        }
        
        Ok(MatchResult {
            found: max_score >= threshold,
            x: best_x,
            y: best_y,
            confidence: max_score,
        })
    }
}

impl GameAutomation {
    pub fn new(
        command_rx: mpsc::Receiver<AutomationCommand>,
        event_tx: mpsc::Sender<AutomationEvent>,
        debug_enabled: bool,
    ) -> Self {
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
            image_config: ImageRecognitionConfig::default(),
        }
    }

    pub async fn initialize_adb(&mut self, use_rust_impl: bool) -> Result<(), String> {
        // Validate template file exists
        if !std::path::Path::new(&self.image_config.template_path).exists() {
            let error = format!("Template image not found: {}", self.image_config.template_path);
            debug_print!(self.debug_enabled, "❌ {}", error);
            let _ = self.event_tx.send(AutomationEvent::Error(error.clone())).await;
            return Err(error);
        }
        debug_print!(self.debug_enabled, "✅ Template image found: {}", self.image_config.template_path);

        match AdbBackend::connect_first(use_rust_impl).await {
            Ok(client) => {
                self.adb_client = Some(Arc::new(Mutex::new(client)));
                debug_print!(self.debug_enabled, "🤖 Game automation ADB client initialized");
                Ok(())
            }
            Err(e) => {
                let error = format!("Failed to initialize ADB for automation: {}", e);
                let _ = self.event_tx.send(AutomationEvent::Error(error.clone())).await;
                Err(error)
            }
        }
    }

    async fn change_state(&mut self, new_state: GameState) {
        if self.state != new_state {
            debug_print!(self.debug_enabled, "🎮 Game automation state: {:?} -> {:?}", self.state, new_state);
            self.state = new_state.clone();
            let _ = self.event_tx.send(AutomationEvent::StateChanged(new_state)).await;
        }
    }

    async fn take_screenshot(&mut self) -> Result<Vec<u8>, String> {
        if let Some(client) = &self.adb_client {
            let client_guard = client.lock().await;
            match client_guard.screen_capture_bytes().await {
                Ok(bytes) => {
                    debug_print!(self.debug_enabled, "📸 Game automation captured screenshot ({} bytes)", bytes.len());
                    
                    // Store the latest screenshot for image recognition
                    self.latest_screenshot = Some(bytes.clone());
                    
                    let _ = self.event_tx.send(AutomationEvent::ScreenshotReady(bytes.clone())).await;
                    Ok(bytes)
                }
                Err(e) => {
                    let error = format!("Screenshot failed: {}", e);
                    let _ = self.event_tx.send(AutomationEvent::Error(error.clone())).await;
                    Err(error)
                }
            }
        } else {
            Err("ADB client not initialized".to_string())
        }
    }

    async fn process_command(&mut self, command: AutomationCommand) {
        debug_print!(self.debug_enabled, "🤖 Processing automation command: {:?}", command);
        match command {
            AutomationCommand::Start => {
                debug_print!(self.debug_enabled, "🤖 Start command received. Current is_running: {}", self.is_running);
                if !self.is_running {
                    self.is_running = true;
                    self.change_state(GameState::WaitingForScreenshot).await;
                    debug_print!(self.debug_enabled, "🚀 Game automation started (interval: {}s)", self.screenshot_interval.as_secs());
                } else {
                    debug_print!(self.debug_enabled, "🤖 Automation already running, ignoring start command");
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
                let _ = self.event_tx.send(AutomationEvent::IntervalUpdate(seconds)).await;
                debug_print!(self.debug_enabled, "⏱️ Screenshot interval updated to {}s", seconds);
            }
            AutomationCommand::TestImageRecognition => {
                debug_print!(self.debug_enabled, "🧪 Manual image recognition test requested");
                if let Err(e) = self.test_image_recognition().await {
                    let _ = self.event_tx.send(AutomationEvent::Error(e)).await;
                }
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
                    debug_print!(self.debug_enabled, "🎮 Entering Acting state - performing image recognition...");
                    
                    // Perform image recognition and actions
                    if let Some(screenshot_bytes) = &self.latest_screenshot {
                        debug_print!(self.debug_enabled, "📸 Screenshot available ({} bytes), analyzing...", screenshot_bytes.len());
                        
                        match self.analyze_and_act(screenshot_bytes).await {
                            Ok(action_taken) => {
                                if action_taken {
                                    debug_print!(self.debug_enabled, "🎯 Game action executed successfully!");
                                    // Wait a bit after taking action before next screenshot
                                    sleep(Duration::from_millis(1000)).await;
                                } else {
                                    debug_print!(self.debug_enabled, "👀 No matching patterns found, continuing scan...");
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
                        debug_print!(self.debug_enabled, "⚠️ No screenshot available for analysis");
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

    /// Update image recognition configuration
    pub fn update_image_config(&mut self, template_path: String, threshold: f32, width: u32, height: u32) {
        self.image_config = ImageRecognitionConfig {
            template_path,
            confidence_threshold: threshold,
            template_width: width,
            template_height: height,
        };
        debug_print!(
            self.debug_enabled,
            "🔧 Image config updated: {} ({}x{}) threshold={:.2}",
            self.image_config.template_path,
            width,
            height,
            threshold
        );
    }

    /// Get current image recognition configuration
    pub fn get_image_config(&self) -> &ImageRecognitionConfig {
        &self.image_config
    }

    /// Manual test of image recognition (for debugging)
    pub async fn test_image_recognition(&self) -> Result<(), String> {
        if let Some(screenshot_bytes) = &self.latest_screenshot {
            debug_print!(self.debug_enabled, "🧪 Testing image recognition with current screenshot...");
            match self.analyze_and_act(screenshot_bytes).await {
                Ok(action_taken) => {
                    if action_taken {
                        debug_print!(self.debug_enabled, "✅ Test completed - action would be taken");
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
        debug_print!(self.debug_enabled, "🔍 Analyzing screenshot for template matches...");
        
        // Perform template matching
        match image_recognition::find_template_in_image(
            screenshot_bytes, 
            &self.image_config.template_path, 
            self.image_config.confidence_threshold
        ) {
            Ok(match_result) => {
                if match_result.found {
                    debug_print!(
                        self.debug_enabled, 
                        "🎯 Template found at ({}, {}) with confidence {:.3}", 
                        match_result.x, 
                        match_result.y, 
                        match_result.confidence
                    );
                    
                    // Calculate tap coordinates at the center of the matched template
                    let tap_x = match_result.x + (self.image_config.template_width / 2);
                    let tap_y = match_result.y + (self.image_config.template_height / 2);
                    
                    // Validate tap coordinates are within screen bounds
                    if let Some(client) = &self.adb_client {
                        let client_guard = client.lock().await;
                        let (screen_width, screen_height) = client_guard.screen_dimensions();
                        
                        if tap_x >= screen_width || tap_y >= screen_height {
                            return Err(format!(
                                "Tap coordinates ({}, {}) are outside screen bounds ({}x{})", 
                                tap_x, tap_y, screen_width, screen_height
                            ));
                        }
                        
                        // Perform the tap action
                        match client_guard.tap(tap_x, tap_y).await {
                            Ok(()) => {
                                debug_print!(
                                    self.debug_enabled,
                                    "✅ Tap executed at ({}, {})", 
                                    tap_x, 
                                    tap_y
                                );
                                return Ok(true); // Action was taken
                            }
                            Err(e) => {
                                return Err(format!("Failed to tap at ({}, {}): {}", tap_x, tap_y, e));
                            }
                        }
                    } else {
                        return Err("ADB client not available for tap action".to_string());
                    }
                } else {
                    debug_print!(
                        self.debug_enabled, 
                        "👀 Template not found (best match: {:.3} < {:.3})", 
                        match_result.confidence, 
                        self.image_config.confidence_threshold
                    );
                    return Ok(false); // No action taken
                }
            }
            Err(e) => {
                return Err(format!("Image analysis failed: {}", e));
            }
        }
    }
}
