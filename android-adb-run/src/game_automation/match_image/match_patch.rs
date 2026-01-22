//! Optimized match-patch algorithm for efficient template matching
//!
//! Implements the fast template matching algorithm from match-patch-regions example with:
//! - Early exit optimization for non-matching patches
//! - Localized search around expected positions
//! - Correlation-based matching with configurable thresholds
//! - Progress reporting for long operations

use image::ImageBuffer;
use image::Luma;

/// Optimized patch matcher using correlation with early exit
pub struct PatchMatcher {
    /// Minimum correlation threshold to continue matching
    threshold: f32,
    /// Maximum number of matches to return
    max_matches: usize,
    /// Search margin around expected position (±N pixels)
    search_margin: u32,
    /// Enable progress reporting
    debug: bool,
}

impl PatchMatcher {
    /// Create a new patch matcher with configuration
    pub fn new(threshold: f32, max_matches: usize, search_margin: u32, debug: bool) -> Self {
        Self {
            threshold,
            max_matches,
            search_margin,
            debug,
        }
    }

    /// Find all matches of a template in an image using optimized correlation
    ///
    /// Returns Vec of (x, y, correlation) tuples sorted by correlation descending
    pub fn find_matches(
        &self,
        image: &ImageBuffer<Luma<u8>, Vec<u8>>,
        template: &ImageBuffer<Luma<u8>, Vec<u8>>,
        expected_x: Option<u32>,
        expected_y: Option<u32>,
    ) -> Vec<(u32, u32, f32)> {
        let image_width = image.width();
        let image_height = image.height();
        let template_width = template.width();
        let template_height = template.height();

        if template_width > image_width || template_height > image_height {
            if self.debug {
                println!(
                    "⚠️ Template {}x{} larger than image {}x{}",
                    template_width, template_height, image_width, image_height
                );
            }
            return Vec::new();
        }

        let mut matches = Vec::new();

        // Determine search region
        let (x_min, x_max, y_min, y_max) = if let (Some(ex), Some(ey)) = (expected_x, expected_y) {
            // Localized search around expected position
            let x_min = ex.saturating_sub(self.search_margin);
            let x_max = (ex + template_width + self.search_margin)
                .min(image_width.saturating_sub(template_width));
            let y_min = ey.saturating_sub(self.search_margin);
            let y_max = (ey + template_height + self.search_margin)
                .min(image_height.saturating_sub(template_height));

            if self.debug {
                println!(
                    "🔍 Localized search: x:[{},{}] y:[{},{}]",
                    x_min, x_max, y_min, y_max
                );
            }

            (x_min, x_max, y_min, y_max)
        } else {
            // Full image search
            if self.debug {
                println!("🔍 Full image search");
            }
            (
                0,
                image_width.saturating_sub(template_width),
                0,
                image_height.saturating_sub(template_height),
            )
        };

        let total_positions = ((x_max - x_min + 1) * (y_max - y_min + 1)) as usize;
        let report_interval = (total_positions / 20).max(1); // Report every 5%

        // Manual pixel-by-pixel search
        for (idx, y) in (y_min..=y_max).enumerate() {
            for (x_idx, x) in (x_min..=x_max).enumerate() {
                let progress_idx = idx * ((x_max - x_min + 1) as usize) + x_idx;

                if self.debug && progress_idx.is_multiple_of(report_interval) {
                    let progress_pct = progress_idx as f32 / total_positions as f32 * 100.0;
                    print!("\r⏳ Search progress: {}%", progress_pct);
                    use std::io::{self, Write};
                    let _ = io::stdout().flush();
                }

                // Extract region from image
                let correlation = self.calculate_correlation_at(image, template, x, y);

                if correlation >= self.threshold {
                    matches.push((x, y, correlation));
                }
            }
        }

        if self.debug {
            println!("\r⏳ Search progress: 100%");
        }

        // Sort by correlation descending and keep only top matches
        matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(self.max_matches);

        if self.debug {
            println!("✅ Found {} matches", matches.len());
        }

        matches
    }

    /// Calculate normalized correlation between template and image region at position
    ///
    /// Uses sum of squared differences normalized to 0.0-1.0 range with early exit
    fn calculate_correlation_at(
        &self,
        image: &ImageBuffer<Luma<u8>, Vec<u8>>,
        template: &ImageBuffer<Luma<u8>, Vec<u8>>,
        x: u32,
        y: u32,
    ) -> f32 {
        let template_width = template.width();
        let template_height = template.height();
        let image_width = image.width();

        if x + template_width > image_width || y + template_height > image.height() {
            return 0.0;
        }

        let mut sum_sq_diff: f64 = 0.0;
        let mut max_possible_sum: f64 = 0.0;

        let template_data = template.as_raw();
        let image_data = image.as_raw();

        let pixels_to_check = (template_width * template_height) as usize;
        let check_interval = (pixels_to_check / 10).max(1); // Check early exit every 10% of pixels

        for py in 0..template_height {
            for px in 0..template_width {
                let template_idx = (py * template_width + px) as usize;
                let image_idx = ((y + py) * image_width + (x + px)) as usize;

                if template_idx >= template_data.len() || image_idx >= image_data.len() {
                    return 0.0;
                }

                let template_val = template_data[template_idx] as f64;
                let image_val = image_data[image_idx] as f64;
                let diff = template_val - image_val;

                sum_sq_diff += diff * diff;
                max_possible_sum += template_val * template_val + image_val * image_val;

                // Early exit: if we can't possibly reach the threshold, stop
                let check_idx = (py * template_width + px) as usize;
                if check_idx > 0
                    && check_idx.is_multiple_of(check_interval)
                    && max_possible_sum > 0.0
                {
                    let current_correlation = 1.0 - (sum_sq_diff / max_possible_sum).min(1.0);
                    let pixels_remaining = pixels_to_check - check_idx;
                    let worst_case_correlation = current_correlation
                        * (1.0 - 0.1 * (pixels_remaining as f64 / pixels_to_check as f64));
                    let threshold_f64 = self.threshold as f64;

                    if worst_case_correlation < threshold_f64 {
                        return 0.0; // Early exit
                    }
                }
            }
        }

        if max_possible_sum > 0.0 {
            (1.0 - (sum_sq_diff / max_possible_sum).min(1.0)) as f32
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_match() {
        let size = 10u32;
        let template_data = vec![0u8; (size * size) as usize];
        let image_data = vec![0u8; (size * size) as usize];

        // Create identical images
        let mut template_data = template_data;
        let mut image_data = image_data;
        for i in 0..(size * size) {
            template_data[i as usize] = 100;
            image_data[i as usize] = 100;
        }

        let template = ImageBuffer::from_raw(size, size, template_data).unwrap();
        let image = ImageBuffer::from_raw(size, size, image_data).unwrap();

        let matcher = PatchMatcher::new(0.9, 1, 0, false);
        let matches = matcher.find_matches(&image, &template, Some(0), Some(0));

        assert!(!matches.is_empty());
        assert!(matches[0].2 > 0.95); // Should be very close to 1.0
    }

    #[test]
    fn test_no_match_different_images() {
        let size = 10u32;
        let template_data = vec![100u8; (size * size) as usize];
        let image_data = vec![50u8; (size * size) as usize];

        let template = ImageBuffer::from_raw(size, size, template_data).unwrap();
        let image = ImageBuffer::from_raw(size, size, image_data).unwrap();

        let matcher = PatchMatcher::new(0.9, 1, 0, false);
        let matches = matcher.find_matches(&image, &template, Some(0), Some(0));

        // Should have no matches with high threshold
        assert!(matches.is_empty() || matches[0].2 < 0.5);
    }

    #[test]
    fn test_localized_search() {
        let size = 20u32;
        let template_data = vec![100u8; 10 * 10];
        let image_data = vec![0u8; (size * size) as usize];

        // Place template pattern in image at position (10, 10)
        let mut image_data = image_data;
        for py in 0..10 {
            for px in 0..10 {
                image_data[(10 + py) * size as usize + (10 + px)] = 100;
            }
        }

        let template = ImageBuffer::from_raw(10, 10, template_data).unwrap();
        let image = ImageBuffer::from_raw(size, size, image_data).unwrap();

        // Search with localized region around expected position
        let matcher = PatchMatcher::new(0.85, 5, 5, false);
        let matches = matcher.find_matches(&image, &template, Some(10), Some(10));

        assert!(!matches.is_empty());
        assert!(matches[0].0 >= 5 && matches[0].0 <= 15);
        assert!(matches[0].1 >= 5 && matches[0].1 <= 15);
    }

    #[test]
    fn test_max_matches_limit() {
        let size = 20u32;
        let template_data = vec![100u8; 5 * 5];
        let image_data = vec![100u8; (size * size) as usize];

        // Create image with template repeated everywhere
        let template = ImageBuffer::from_raw(5, 5, template_data).unwrap();
        let image = ImageBuffer::from_raw(size, size, image_data).unwrap();

        let matcher = PatchMatcher::new(0.95, 3, 0, false);
        let matches = matcher.find_matches(&image, &template, None, None);

        // Should return at most 3 matches
        assert!(matches.len() <= 3);
    }
}
