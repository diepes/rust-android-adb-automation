//! Configuration for image matching operations

#[derive(Debug, Clone)]
pub struct MatchConfig {
    /// Confidence threshold for template matching (0.0 to 1.0)
    pub confidence_threshold: f32,
    /// Maximum number of matches to return per template
    pub max_matches_per_template: usize,
    /// Whether to use multi-scale matching
    pub enable_multiscale: bool,
    /// Scale factors for multi-scale matching
    pub scale_factors: Vec<f32>,
    /// Debug mode flag
    pub debug_enabled: bool,
    /// Use optimized match-patch algorithm with early exit
    pub use_match_patch_optimization: bool,
    /// Search margin for localized match-patch search (±N pixels)
    pub match_patch_search_margin: u32,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.8,
            max_matches_per_template: 1,
            enable_multiscale: false,
            scale_factors: vec![0.8, 0.9, 1.0, 1.1, 1.2],
            debug_enabled: false,
            use_match_patch_optimization: false,
            match_patch_search_margin: 10,
        }
    }
}

/// Create a default configuration for game automation
pub fn create_default_config() -> MatchConfig {
    MatchConfig {
        confidence_threshold: 0.85,
        max_matches_per_template: 3,
        enable_multiscale: true,
        scale_factors: vec![0.9, 1.0, 1.1],
        debug_enabled: false,
        use_match_patch_optimization: false,
        match_patch_search_margin: 10,
    }
}

/// Configuration preset for UI elements (buttons, menus)
pub fn create_ui_config() -> MatchConfig {
    MatchConfig {
        confidence_threshold: 0.9,
        max_matches_per_template: 1,
        enable_multiscale: false,
        scale_factors: vec![1.0],
        debug_enabled: false,
        use_match_patch_optimization: true,
        match_patch_search_margin: 20,
    }
}

/// Configuration preset for game objects (items, characters)
pub fn create_game_object_config() -> MatchConfig {
    MatchConfig {
        confidence_threshold: 0.75,
        max_matches_per_template: 5,
        enable_multiscale: true,
        scale_factors: vec![0.8, 0.9, 1.0, 1.1, 1.2],
        debug_enabled: false,
        use_match_patch_optimization: false,
        match_patch_search_margin: 50,
    }
}
