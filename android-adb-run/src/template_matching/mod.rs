/// Template matching module for patch detection in screenshots
///
/// This module provides efficient template matching with:
/// - Early exit optimization for non-matching patches
/// - Localized search around expected positions
/// - Progress reporting for long operations
/// - Correlation-based matching with configurable thresholds
pub mod matcher;
pub mod types;

pub use matcher::TemplateMatcher;
pub use types::{Match, PatchInfo};
