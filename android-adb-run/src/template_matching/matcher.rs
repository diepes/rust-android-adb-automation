/// Template matching implementation
///
/// Optimized correlation-based matching with early exit optimization

use super::types::{Match, PatchInfo};
use image::RgbImage;

/// Template matcher for finding patches in images
pub struct TemplateMatcher {
    patches: Vec<PatchInfo>,
}

impl TemplateMatcher {
    /// Create a new empty matcher
    pub fn new() -> Self {
        Self {
            patches: Vec::new(),
        }
    }

    /// Add a patch to the matcher
    pub fn add_patch(&mut self, patch: PatchInfo) {
        self.patches.push(patch);
    }

    /// Get all loaded patches
    pub fn patches(&self) -> &[PatchInfo] {
        &self.patches
    }

    /// Clear all patches
    pub fn clear(&mut self) {
        self.patches.clear();
    }

    /// Find best matches for a patch in an image
    ///
    /// # Arguments
    /// * `image_rgb` - The image to search in (RGB format)
    /// * `patch_idx` - Index of the patch to find
    /// * `threshold` - Correlation threshold (0.0-1.0)
    /// * `max_matches` - Maximum number of matches to return
    /// * `search_margin` - Search region margin around expected position (±N pixels)
    ///
    /// # Returns
    /// Vec of matches sorted by correlation (highest first)
    pub fn find_matches(
        &self,
        image_rgb: &RgbImage,
        patch_idx: usize,
        threshold: f32,
        max_matches: usize,
        _search_margin: u32,
    ) -> Vec<Match> {
        if patch_idx >= self.patches.len() {
            return Vec::new();
        }

        let patch = &self.patches[patch_idx];
        let image_width = image_rgb.width() as u32;
        let image_height = image_rgb.height() as u32;

        // Convert patch pixels to RgbImage
        let patch_img = match RgbImage::from_raw(
            patch.width,
            patch.height,
            patch.pixels.clone(),
        ) {
            Some(img) => img,
            None => return Vec::new(),
        };

        // Determine search region
        let (x_min, x_max, y_min, y_max) = (
            0_u32,
            image_width.saturating_sub(patch.width),
            0_u32,
            image_height.saturating_sub(patch.height),
        );

        let mut matches: Vec<Match> = Vec::new();

        // Calculate total positions for progress reporting
        let total_y = (y_max - y_min + 1) as usize;
        let total_x = (x_max - x_min + 1) as usize;
        let total_positions = total_y * total_x;
        let report_interval = (total_positions / 10).max(1); // Report every 10%
        let mut position_count = 0;

        // Search through possible positions
        for y in y_min..=y_max {
            for x in x_min..=x_max {
                // Extract region from image
                if let Some(region) = self.extract_region(image_rgb, x, y, patch.width, patch.height) {
                    let corr = self.calculate_correlation(&patch_img, &region, threshold);

                    if corr >= threshold {
                        matches.push(Match {
                            x,
                            y,
                            correlation: corr,
                        });
                    }
                }

                position_count += 1;
                if position_count % report_interval == 0 {
                    let progress_pct = (position_count as f32 / total_positions as f32 * 100.0) as u32;
                    log::debug!("  ⏳ Correlation scanning: {}%", progress_pct);
                }
            }
        }

        // Sort by correlation descending
        matches.sort_by(|a, b| b.correlation.partial_cmp(&a.correlation).unwrap());
        matches.truncate(max_matches);

        matches
    }

    /// Extract a region from an image
    fn extract_region(
        &self,
        image: &RgbImage,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Option<RgbImage> {
        if x + width > image.width() || y + height > image.height() {
            return None;
        }

        let mut region = RgbImage::new(width, height);
        for dy in 0..height {
            for dx in 0..width {
                if let Some(pixel) = image.get_pixel_checked(x + dx, y + dy) {
                    region.put_pixel(dx, dy, *pixel);
                } else {
                    return None;
                }
            }
        }

        Some(region)
    }

    /// Calculate normalized correlation between patch and region
    ///
    /// Uses sum of squared differences normalized to 0.0-1.0 range
    fn calculate_correlation(
        &self,
        patch: &RgbImage,
        region: &RgbImage,
        min_match: f32,
    ) -> f32 {
        if patch.width() != region.width() || patch.height() != region.height() {
            return 0.0;
        }

        let pixel_count = (patch.width() * patch.height()) as usize;
        if pixel_count == 0 {
            return 0.0;
        }

        // Calculate max possible difference (all pixels completely different)
        // Each pixel has 3 channels (R,G,B), max difference per channel is 255
        let max_sq_diff = (pixel_count as f64) * 3.0 * (255.0 * 255.0);
        let max_allowed_diff = max_sq_diff * (1.0 - min_match as f64);

        let mut sum_sq_diff = 0.0;
        let mut checked_pixels = 0;

        for (p_pixel, r_pixel) in patch.pixels().zip(region.pixels()) {
            let p_data = p_pixel.0;
            let r_data = r_pixel.0;

            for i in 0..3 {
                let diff = (p_data[i] as i32) - (r_data[i] as i32);
                sum_sq_diff += (diff * diff) as f64;
            }

            checked_pixels += 1;

            // Early exit optimization: periodically check if threshold can still be met
            if checked_pixels % 1000 == 0 && sum_sq_diff > max_allowed_diff {
                return 0.0;
            }
        }

        // Convert to 0.0-1.0 correlation score
        let correlation = 1.0 - (sum_sq_diff / max_sq_diff);
        (correlation.max(0.0).min(1.0)) as f32
    }
}

impl Default for TemplateMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_matcher() {
        let matcher = TemplateMatcher::new();
        assert_eq!(matcher.patches().len(), 0);
    }

    #[test]
    fn test_add_patch() {
        let mut matcher = TemplateMatcher::new();
        let patch = PatchInfo::new(
            Some("test".to_string()),
            100,
            100,
            50,
            50,
            vec![0; 50 * 50 * 3],
        );
        matcher.add_patch(patch);
        assert_eq!(matcher.patches().len(), 1);
    }

    #[test]
    fn test_correlation_perfect_match() {
        let matcher = TemplateMatcher::new();
        let pixels = vec![100u8; 300]; // 10x10 RGB = 300 bytes
        let patch = RgbImage::from_raw(10, 10, pixels.clone()).unwrap();
        let region = RgbImage::from_raw(10, 10, pixels).unwrap();

        let corr = matcher.calculate_correlation(&patch, &region, 0.9);
        assert!(corr >= 0.99, "Perfect match should have correlation >= 0.99");
    }

    #[test]
    fn test_size_mismatch() {
        let matcher = TemplateMatcher::new();
        let patch = RgbImage::from_raw(10, 10, vec![100u8; 300]).unwrap();
        let region = RgbImage::from_raw(20, 20, vec![100u8; 1200]).unwrap();

        let corr = matcher.calculate_correlation(&patch, &region, 0.9);
        assert_eq!(corr, 0.0, "Size mismatch should return 0.0");
    }
}
