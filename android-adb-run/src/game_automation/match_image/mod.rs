//! Image matching module for Android game automation
//!
//! This module provides specialized image recognition capabilities for Android games,
//! including template matching, region-based searching, and game state detection.

pub mod config;
pub mod detector;
pub mod region;
pub mod template;

#[cfg(test)]
mod tests;

// Re-export main types and functions
pub use config::{MatchConfig, create_default_config, create_game_object_config, create_ui_config};
pub use detector::{DetectionResult, GameStateDetector};
pub use region::{RegionManager, SearchRegion};
pub use template::{Template, TemplateCategory, TemplateManager, TemplateMatch};
