//! Game state detection and image matching engine

use super::{
    config::MatchConfig,
    template::{Template, TemplateCategory, TemplateManager, TemplateMatch},
};
use crate::game_automation::types::GameState;
use image::{ImageBuffer, Luma};
use imageproc::template_matching::{MatchTemplateMethod, match_template};

#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub matches: Vec<TemplateMatch>,
    pub suggested_state: Option<GameState>,
    pub confidence_score: f32,
    pub processing_time_ms: u128,
}

impl DetectionResult {
    pub fn new() -> Self {
        Self {
            matches: Vec::new(),
            suggested_state: None,
            confidence_score: 0.0,
            processing_time_ms: 0,
        }
    }

    pub fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    pub fn best_match(&self) -> Option<&TemplateMatch> {
        self.matches.iter().max_by(|a, b| {
            a.confidence
                .partial_cmp(&b.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Main game state detector that performs image matching and analysis
pub struct GameStateDetector {
    template_manager: TemplateManager,
    config: MatchConfig,
    screen_width: u32,
    screen_height: u32,
}

impl GameStateDetector {
    pub fn new(screen_width: u32, screen_height: u32, config: MatchConfig) -> Self {
        Self {
            template_manager: TemplateManager::new(screen_width, screen_height),
            config,
            screen_width,
            screen_height,
        }
    }

    /// Load templates from directory
    pub fn load_templates(&mut self, directory: &str) -> Result<usize, String> {
        self.template_manager
            .load_templates_from_directory(directory)
    }

    /// Analyze screenshot and detect game state
    pub fn analyze_screenshot(&self, screenshot_bytes: &[u8]) -> Result<DetectionResult, String> {
        let start_time = std::time::Instant::now();

        // Load screenshot image
        let screenshot = image::load_from_memory(screenshot_bytes)
            .map_err(|e| format!("Failed to load screenshot: {e}"))?;
        let screenshot_gray = screenshot.to_luma8();

        let mut result = DetectionResult::new();

        // Process each template
        for (i, template) in self.template_manager.get_templates().iter().enumerate() {
            if self.config.debug_enabled {
                println!("🔍 Processing template {}/{}: {}", i + 1, self.template_manager.get_templates().len(), template.name);
            }
            
            match self.match_template_in_region(&screenshot_gray, template) {
                Ok(matches) => {
                    if self.config.debug_enabled && !matches.is_empty() {
                        println!("✅ Found {} matches for template '{}'", matches.len(), template.name);
                    }
                    result.matches.extend(matches);
                }
                Err(e) => {
                    if self.config.debug_enabled {
                        println!("❌ Template matching failed for '{}': {}", template.name, e);
                    }
                }
            }
        }

        // Sort matches by confidence
        result.matches.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Determine suggested game state based on matches
        result.suggested_state = self.determine_game_state(&result.matches);

        // Calculate overall confidence
        result.confidence_score = self.calculate_overall_confidence(&result.matches);

        result.processing_time_ms = start_time.elapsed().as_millis();

        if self.config.debug_enabled {
            self.log_detection_results(&result);
        }

        Ok(result)
    }

    /// Match a single template within its search region
    fn match_template_in_region(
        &self,
        screenshot_gray: &ImageBuffer<Luma<u8>, Vec<u8>>,
        template: &Template,
    ) -> Result<Vec<TemplateMatch>, String> {
        if self.config.debug_enabled {
            println!("🔍 Loading template: {} (search region at {},{} {}x{})", 
                template.name, 
                template.search_region.x,
                template.search_region.y,
                template.search_region.width,
                template.search_region.height
            );
        }
        
        // Load and crop template image to the region specified in filename
        let template_gray = self.load_and_crop_template(template)?;

        let mut matches = Vec::new();

        // Crop screenshot to search region
        let region = &template.search_region;
        if region.x + region.width > self.screen_width
            || region.y + region.height > self.screen_height
        {
            return Err(format!("Search region exceeds screen bounds"));
        }

        let cropped_view = image::imageops::crop_imm(
            screenshot_gray,
            region.x,
            region.y,
            region.width,
            region.height,
        );

        // Convert SubImage to ImageBuffer
        let cropped = cropped_view.to_image();

        if self.config.enable_multiscale {
            // Multi-scale matching
            for &scale in &self.config.scale_factors {
                if let Ok(scaled_matches) =
                    self.match_at_scale(&cropped, &template_gray, template, scale, region)
                {
                    matches.extend(scaled_matches);
                }
            }
        } else {
            // Single-scale matching
            if let Ok(single_matches) =
                self.match_at_scale(&cropped, &template_gray, template, 1.0, region)
            {
                matches.extend(single_matches);
            }
        }

        // Keep only the best matches
        matches.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(self.config.max_matches_per_template);

        Ok(matches)
    }

    /// Perform template matching at a specific scale
    fn match_at_scale(
        &self,
        cropped_screenshot: &ImageBuffer<Luma<u8>, Vec<u8>>,
        template_gray: &ImageBuffer<Luma<u8>, Vec<u8>>,
        template: &Template,
        scale: f32,
        region: &super::region::SearchRegion,
    ) -> Result<Vec<TemplateMatch>, String> {
        let mut matches = Vec::new();

        let scaled_template = if (scale - 1.0).abs() > 0.01 {
            // Scale template if needed
            let new_width = (template_gray.width() as f32 * scale) as u32;
            let new_height = (template_gray.height() as f32 * scale) as u32;

            if new_width == 0 || new_height == 0 {
                return Ok(matches);
            }

            image::imageops::resize(
                template_gray,
                new_width,
                new_height,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            template_gray.clone()
        };

        // Skip if scaled template is larger than search area
        if scaled_template.width() > cropped_screenshot.width()
            || scaled_template.height() > cropped_screenshot.height()
        {
            if self.config.debug_enabled {
                println!("⚠️ Skipping template - too large for region: {}x{} > {}x{}", 
                    scaled_template.width(), scaled_template.height(),
                    cropped_screenshot.width(), cropped_screenshot.height());
            }
            return Ok(matches);
        }

        // Additional safety check for very large templates that could cause hangs
        let template_pixels = scaled_template.width() as u64 * scaled_template.height() as u64;
        let search_pixels = cropped_screenshot.width() as u64 * cropped_screenshot.height() as u64;
        
        if template_pixels > 1_000_000 || search_pixels > 5_000_000 {
            if self.config.debug_enabled {
                println!("⚠️ Skipping large template matching to prevent hang: template={}x{}, search={}x{}", 
                    scaled_template.width(), scaled_template.height(),
                    cropped_screenshot.width(), cropped_screenshot.height());
            }
            return Ok(matches);
        }

        if self.config.debug_enabled {
            println!("🔍 Performing template matching: {}x{} in {}x{} region", 
                scaled_template.width(), scaled_template.height(),
                cropped_screenshot.width(), cropped_screenshot.height());
        }

        // Perform template matching
        let result = match_template(
            cropped_screenshot,
            &scaled_template,
            MatchTemplateMethod::CrossCorrelationNormalized,
        );

        // Find matches above threshold
        for (x, y, pixel) in result.enumerate_pixels() {
            let confidence = pixel[0] as f32 / 255.0;

            if confidence >= self.config.confidence_threshold {
                // Convert coordinates back to screen space
                let screen_x = region.x + x;
                let screen_y = region.y + y;

                let template_match =
                    TemplateMatch::new(template.clone(), screen_x, screen_y, confidence, scale);

                if template_match.is_within_bounds(self.screen_width, self.screen_height) {
                    matches.push(template_match);
                }
            }
        }

        Ok(matches)
    }

    /// Determine game state based on detected matches
    fn determine_game_state(&self, matches: &[TemplateMatch]) -> Option<GameState> {
        if matches.is_empty() {
            return Some(GameState::WaitingForScreenshot);
        }

        // Analyze matches to suggest game state
        // This is where game-specific logic would go

        let best_match = matches.first()?;

        // Example state determination logic
        match best_match.template.category {
            TemplateCategory::Button => {
                if best_match.confidence > 0.9 {
                    Some(GameState::Acting)
                } else {
                    Some(GameState::Analyzing)
                }
            }
            _ => Some(GameState::Analyzing),
        }
    }

    /// Calculate overall confidence score
    fn calculate_overall_confidence(&self, matches: &[TemplateMatch]) -> f32 {
        if matches.is_empty() {
            return 0.0;
        }

        // Weight the confidence by match quality
        let total_confidence: f32 = matches
            .iter()
            .take(3) // Consider top 3 matches
            .enumerate()
            .map(|(i, m)| m.confidence * (1.0 / (i as f32 + 1.0))) // Decreasing weight
            .sum();

        let weight_sum: f32 = (0..matches.len().min(3))
            .map(|i| 1.0 / (i as f32 + 1.0))
            .sum();

        total_confidence / weight_sum
    }

    /// Log detection results for debugging
    fn log_detection_results(&self, result: &DetectionResult) {
        println!("🔍 Detection Results:");
        println!("  Processing time: {}ms", result.processing_time_ms);
        println!("  Overall confidence: {:.3}", result.confidence_score);
        println!("  Matches found: {}", result.matches.len());

        for (i, m) in result.matches.iter().take(5).enumerate() {
            println!(
                "    {}. {} at ({},{}) conf={:.3} scale={:.2}",
                i + 1,
                m.template.name,
                m.x,
                m.y,
                m.confidence,
                m.scale_factor
            );
        }

        if let Some(state) = &result.suggested_state {
            println!("  Suggested state: {:?}", state);
        }
    }

    /// Update configuration
    pub fn update_config(&mut self, config: MatchConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> &MatchConfig {
        &self.config
    }

    /// Reload templates
    pub fn reload_templates(&mut self, directory: &str) -> Result<usize, String> {
        self.template_manager.reload_templates(directory)
    }

    /// Get template count
    pub fn get_template_count(&self) -> usize {
        self.template_manager.count()
    }

    /// Load template image and crop it to the region specified in the filename
    /// For files like "img-[300,1682,50,50].png", this extracts the 50x50 region 
    /// at coordinates (300,1682) from the full screenshot stored in the file
    fn load_and_crop_template(
        &self,
        template: &Template,
    ) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>, String> {
        // Load the full template image (which may be a full screenshot)
        let template_image = image::open(&template.path)
            .map_err(|e| format!("Failed to load template {}: {e}", template.path))?;
        
        // Check if filename contains region coordinates [x,y,width,height]
        if let Some(region_coords) = self.extract_template_region_from_filename(&template.name) {
            if self.config.debug_enabled {
                println!("📐 Cropping template '{}' from full image ({}x{}) to region: [{},{},{},{}]", 
                    template.name,
                    template_image.width(), 
                    template_image.height(),
                    region_coords.0, region_coords.1, region_coords.2, region_coords.3
                );
            }
            
            // Validate crop region bounds
            let (crop_x, crop_y, crop_w, crop_h) = region_coords;
            if crop_x + crop_w > template_image.width() || crop_y + crop_h > template_image.height() {
                return Err(format!(
                    "Template crop region [{},{},{},{}] exceeds image bounds ({}x{})",
                    crop_x, crop_y, crop_w, crop_h, 
                    template_image.width(), template_image.height()
                ));
            }
            
            // Crop the template to the specified region
            let cropped = image::imageops::crop_imm(&template_image, crop_x, crop_y, crop_w, crop_h);
            let cropped_dynamic = image::DynamicImage::ImageRgba8(cropped.to_image());
            let cropped_gray = cropped_dynamic.to_luma8();
            
            if self.config.debug_enabled {
                println!("✂️ Template '{}' cropped to {}x{} (was {}x{})", 
                    template.name, 
                    cropped_gray.width(), cropped_gray.height(),
                    template_image.width(), template_image.height()
                );
            }
            
            Ok(cropped_gray)
        } else {
            // No region specified in filename, use full image
            let template_gray = template_image.to_luma8();
            
            // Still warn if template is very large
            if template_gray.width() > 500 || template_gray.height() > 500 {
                if self.config.debug_enabled {
                    println!("⚠️ Large template detected: {}x{} - this may be slow!", 
                        template_gray.width(), template_gray.height());
                }
            }
            
            Ok(template_gray)
        }
    }

    /// Extract template region coordinates from filename
    /// Returns (x, y, width, height) if found, None otherwise
    fn extract_template_region_from_filename(&self, filename: &str) -> Option<(u32, u32, u32, u32)> {
        // Look for pattern [x,y,width,height] in filename
        if let Some(start) = filename.find('[') {
            if let Some(end) = filename.find(']') {
                if end > start {
                    let region_str = &filename[start + 1..end];
                    let parts: Vec<&str> = region_str.split(',').collect();
                    if parts.len() == 4 {
                        if let (Ok(x), Ok(y), Ok(width), Ok(height)) = (
                            parts[0].trim().parse::<u32>(),
                            parts[1].trim().parse::<u32>(),
                            parts[2].trim().parse::<u32>(),
                            parts[3].trim().parse::<u32>(),
                        ) {
                            return Some((x, y, width, height));
                        }
                    }
                }
            }
        }
        None
    }
}
