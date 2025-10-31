// Game automation module
// This module provides a finite state machine for automating game interactions
// with Android devices via ADB.

pub mod types;
pub mod fsm;
pub mod channels;

// Re-export the main types and functions for easy access
pub use types::{GameState, AutomationCommand, AutomationEvent};
pub use fsm::GameAutomation;
pub use channels::create_automation_channels;
