// Game automation module
// This module provides a finite state machine for automating game interactions
// with Android devices via ADB.

pub mod channels;
pub mod fsm;
pub mod match_image;
pub mod types;

// Re-export the main types and functions for easy access
pub use channels::create_automation_channels;
pub use fsm::GameAutomation;
pub use match_image::{DetectionResult, GameStateDetector, MatchConfig, Template, TemplateMatch};
pub use types::{AutomationCommand, AutomationEvent, GameState};
