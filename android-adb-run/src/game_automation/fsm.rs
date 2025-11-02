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
    template_paths: Vec<String>,
    confidence_threshold: f32,
}

impl Default for ImageRecognitionConfig {
    fn default() -> Self {
        Self {
            template_paths: Vec::new(), // Will be populated by scanning for *.png files
            confidence_threshold: 0.8,
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
        pub template_path: String,
        pub template_width: u32,
        pub template_height: u32,
    }
    
    /// Find templates in image, checking all provided template paths
    pub fn find_templates_in_image(
        screenshot_bytes: &[u8], 
        template_paths: &[String],
        threshold: f32,
    ) -> Result<MatchResult, String> {
        // Load the screenshot from PNG bytes
        let screenshot = image::load_from_memory(screenshot_bytes)
            .map_err(|e| format!("Failed to load screenshot: {e}"))?;
        let screenshot_gray = screenshot.to_luma8();
        
        let mut best_match = MatchResult {
            found: false,
            x: 0,
            y: 0,
            confidence: 0.0,
            template_path: String::new(),
            template_width: 0,
            template_height: 0,
        };
        
        // Try each template and find the best match across all
        for template_path in template_paths {
            match process_single_template(&screenshot_gray, template_path, threshold) {
                Ok(mut result) => {
                    if result.found && result.confidence > best_match.confidence {
                        result.template_path = template_path.clone();
                        best_match = result;
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ Failed to process template {}: {}", template_path, e);
                    continue;
                }
            }
        }
        
        Ok(best_match)
    }
    
    fn process_single_template(
        screenshot_gray: &image::ImageBuffer<image::Luma<u8>, Vec<u8>>,
        template_path: &str,
        threshold: f32,
    ) -> Result<MatchResult, String> {
        // Load the template image
        let template = image::open(template_path)
            .map_err(|e| format!("Failed to load template {}: {e}", template_path))?;
        
        let template_gray = template.to_luma8();
        let template_width = template_gray.width();
        let template_height = template_gray.height();
        
        // Perform template matching using normalized cross correlation
        let result = match_template(screenshot_gray, &template_gray, MatchTemplateMethod::CrossCorrelationNormalized);
        
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
            template_path: template_path.to_string(),
            template_width,
            template_height,
        })
    }
    
    /// Scan current directory for PNG template files
    pub fn scan_template_files() -> Result<Vec<String>, String> {
        let mut template_paths = Vec::new();
        
        let current_dir = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;
            
        let entries = std::fs::read_dir(&current_dir)
            .map_err(|e| format!("Failed to read directory: {}", e))?;
            
        for entry in entries {
            if let Ok(entry) = entry {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with(".png") && entry.path().is_file() {
                        template_paths.push(file_name.to_string());
                    }
                }
            }
        }
        
        // Sort for consistent ordering
        template_paths.sort();
        
        if template_paths.is_empty() {
            return Err("No PNG template files found in current directory".to_string());
        }
        
        Ok(template_paths)
    }
}

impl GameAutomation {
    pub fn new(
        command_rx: mpsc::Receiver<AutomationCommand>,
        event_tx: mpsc::Sender<AutomationEvent>,
        debug_enabled: bool,
    ) -> Self {
        let mut image_config = ImageRecognitionConfig::default();
        
        // Scan for template files on initialization  
        match image_recognition::scan_template_files() {
            Ok(template_paths) => {
                image_config.template_paths = template_paths;
            }
            Err(e) => {
                eprintln!("⚠️ Failed to scan template files: {}", e);
                // Continue with empty template list, will be handled during initialization
            }
        }
        
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
            image_config,
        }
    }

    pub async fn initialize_adb(&mut self, use_rust_impl: bool) -> Result<(), String> {
        // Rescan template files in case new ones were added
        match image_recognition::scan_template_files() {
            Ok(template_paths) => {
                self.image_config.template_paths = template_paths.clone();
                debug_print!(self.debug_enabled, "✅ Found {} template files: {:?}", template_paths.len(), template_paths);
                // Notify GUI about initial template discovery
                let _ = self.event_tx.send(AutomationEvent::TemplatesUpdated(template_paths)).await;
            }
            Err(e) => {
                let error = format!("No template images found: {}", e);
                debug_print!(self.debug_enabled, "❌ {}", error);
                let _ = self.event_tx.send(AutomationEvent::Error(error.clone())).await;
                return Err(error);
            }
        }

        // Validate that template files exist
        for template_path in &self.image_config.template_paths {
            if !std::path::Path::new(template_path).exists() {
                let error = format!("Template image not found: {}", template_path);
                debug_print!(self.debug_enabled, "❌ {}", error);
                let _ = self.event_tx.send(AutomationEvent::Error(error.clone())).await;
                return Err(error);
            }
        }

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
            AutomationCommand::RescanTemplates => {
                debug_print!(self.debug_enabled, "🔄 Template rescan requested");
                if let Err(e) = self.rescan_templates().await {
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
    pub fn update_image_config(&mut self, template_paths: Vec<String>, threshold: f32) {
        self.image_config = ImageRecognitionConfig {
            template_paths: template_paths.clone(),
            confidence_threshold: threshold,
        };
        debug_print!(
            self.debug_enabled,
            "🔧 Image config updated: {} templates, threshold={:.2}",
            template_paths.len(),
            threshold
        );
    }

    /// Rescan template files and update configuration
    pub async fn rescan_templates(&mut self) -> Result<(), String> {
        match image_recognition::scan_template_files() {
            Ok(template_paths) => {
                self.image_config.template_paths = template_paths.clone();
                debug_print!(
                    self.debug_enabled,
                    "🔄 Rescanned templates: found {} files: {:?}",
                    template_paths.len(),
                    template_paths
                );
                // Notify GUI about template update
                let _ = self.event_tx.send(AutomationEvent::TemplatesUpdated(template_paths)).await;
                Ok(())
            }
            Err(e) => {
                debug_print!(self.debug_enabled, "❌ Template rescan failed: {}", e);
                Err(e)
            }
        }
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
        if self.image_config.template_paths.is_empty() {
            return Err("No template images configured for matching".to_string());
        }

        debug_print!(
            self.debug_enabled, 
            "🔍 Analyzing screenshot for template matches across {} templates...", 
            self.image_config.template_paths.len()
        );
        
        // Perform template matching across all templates
        match image_recognition::find_templates_in_image(
            screenshot_bytes, 
            &self.image_config.template_paths, 
            self.image_config.confidence_threshold
        ) {
            Ok(match_result) => {
                if match_result.found {
                    debug_print!(
                        self.debug_enabled, 
                        "🎯 Template '{}' found at ({}, {}) with confidence {:.3}", 
                        match_result.template_path,
                        match_result.x, 
                        match_result.y, 
                        match_result.confidence
                    );
                    
                    // Calculate tap coordinates at the center of the matched template
                    let tap_x = match_result.x + (match_result.template_width / 2);
                    let tap_y = match_result.y + (match_result.template_height / 2);
                    
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
                                    "✅ Tap executed at ({}, {}) for template '{}'", 
                                    tap_x, 
                                    tap_y,
                                    match_result.template_path
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
                        "👀 No templates matched (best confidence: {:.3} < {:.3})", 
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
