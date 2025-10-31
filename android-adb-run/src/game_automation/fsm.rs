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

pub struct GameAutomation {
    state: GameState,
    screenshot_interval: Duration,
    adb_client: Option<Arc<Mutex<AdbBackend>>>,
    command_rx: mpsc::Receiver<AutomationCommand>,
    event_tx: mpsc::Sender<AutomationEvent>,
    is_running: bool,
    should_exit: bool,
    debug_enabled: bool,
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
        }
    }

    pub async fn initialize_adb(&mut self, use_rust_impl: bool) -> Result<(), String> {
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
                    // Placeholder for game action logic (taps, swipes, etc.)
                    // For now, just go back to waiting
                    sleep(Duration::from_millis(500)).await; // Simulate processing time
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
}
