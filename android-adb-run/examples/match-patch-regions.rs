//! Match patch regions against full screenshots using imageproc template matching
//! Validates template matching: loading patches and searching for matches in full images
//! Usage: cargo run --release --example match-patch-regions
//! 
//! This version uses imageproc::template_matching for fast, optimized template matching

#[allow(unused_imports)]
use image::GenericImageView;
use imageproc::template_matching::{match_template, MatchTemplateMethod};
use std::fs;
use std::path::Path;
use std::time::Instant;

/// Extract region coordinates and label from source filename
fn extract_region_and_label_from_filename(source_path: &str) -> Option<(Option<String>, u32, u32, u32, u32)> {
    let path = Path::new(source_path);
    let filename = path.file_name()?.to_string_lossy();
    
    if let Some(bracket_start) = filename.find('[') {
        if let Some(bracket_end) = filename.find(']') {
            let coords_str = &filename[bracket_start + 1..bracket_end];
            let parts: Vec<&str> = coords_str.split(',').collect();
            
            if parts.len() == 4 {
                if let (Ok(x), Ok(y), Ok(width), Ok(height)) = (
                    parts[0].trim().parse::<u32>(),
                    parts[1].trim().parse::<u32>(),
                    parts[2].trim().parse::<u32>(),
                    parts[3].trim().parse::<u32>(),
                ) {
                    let label = if bracket_start > 7 {  // "patch-[" is 7 chars
                        let label_part = &filename[6..bracket_start].trim_end_matches('-');
                        if label_part.is_empty() {
                            None
                        } else {
                            Some(label_part.to_string())
                        }
                    } else {
                        None
                    };
                    
                    return Some((label, x, y, width, height));
                }
            }
        }
    }
    
    None
}

/// Calculate correlation using imageproc template matching (SSD method)
/// Returns 0.0 to 1.0 where 1.0 is perfect match
fn calculate_correlation(patch: &image::RgbImage, region: &image::RgbImage) -> f32 {
    if patch.width() != region.width() || patch.height() != region.height() {
        return 0.0;
    }
    
    // For same-size regions, compute SSD (sum of squared differences)
    let mut sum_sq_diff: f64 = 0.0;
    for (p_pixel, r_pixel) in patch.pixels().zip(region.pixels()) {
        let r_diff = (p_pixel[0] as f64 - r_pixel[0] as f64);
        let g_diff = (p_pixel[1] as f64 - r_pixel[1] as f64);
        let b_diff = (p_pixel[2] as f64 - r_pixel[2] as f64);
        sum_sq_diff += r_diff * r_diff + g_diff * g_diff + b_diff * b_diff;
    }
    
    let pixels = (patch.width() * patch.height()) as f64;
    let max_sq_diff = pixels * 255.0 * 255.0 * 3.0;
    
    if max_sq_diff == 0.0 {
        return 1.0;
    }
    
    let correlation = 1.0 - (sum_sq_diff / max_sq_diff);
    correlation.max(0.0).min(1.0) as f32
}

/// Find matches using a hybrid approach: imageproc for full image, manual scan for local regions
fn find_matches(
    image: &image::DynamicImage, 
    patch: &image::RgbImage, 
    threshold: f32, 
    max_matches: u32,
    expected_x: Option<u32>,
    expected_y: Option<u32>,
    search_margin: u32,
) -> Vec<(u32, u32, f32)> {
    let image_rgb = image.to_rgb8();
    let patch_width = patch.width();
    let patch_height = patch.height();
    let image_width = image_rgb.width();
    let image_height = image_rgb.height();
    
    if patch_width > image_width || patch_height > image_height {
        return Vec::new();
    }
    
    let mut matches = Vec::new();
    
    eprint!("        ⏳ Search progress: ");
    
    // If we have expected location, search locally (faster)
    if let (Some(ex), Some(ey)) = (expected_x, expected_y) {
        let x_min = ex.saturating_sub(search_margin);
        let x_max = (ex + patch_width + search_margin).min(image_width.saturating_sub(patch_width));
        let y_min = ey.saturating_sub(search_margin);
        let y_max = (ey + patch_height + search_margin).min(image_height.saturating_sub(patch_height));
        
        // Manual pixel-by-pixel search in localized region (much faster than full template matching)
        for y in y_min..=y_max {
            for x in x_min..=x_max {
                if x + patch_width <= image_width && y + patch_height <= image_height {
                    let region = image::RgbImage::from_fn(patch_width, patch_height, |px, py| {
                        image_rgb.get_pixel(x + px, y + py).clone()
                    });
                    
                    let correlation = calculate_correlation(patch, &region);
                    
                    if correlation >= threshold {
                        matches.push((x, y, correlation));
                    }
                }
            }
        }
    } else {
        // Full image: use imageproc for full-image search
        let image_gray = image::imageops::grayscale(&image::DynamicImage::ImageRgb8(image_rgb));
        let patch_gray = image::imageops::grayscale(&image::DynamicImage::ImageRgb8(patch.clone()));
        
        let result = match_template(
            &image_gray, 
            &patch_gray, 
            MatchTemplateMethod::CrossCorrelationNormalized
        );
        
        for y in 0..result.height() {
            for x in 0..result.width() {
                if let Some(pixel) = result.get_pixel_checked(x, y) {
                    let ncc_value = pixel[0];
                    let correlation = (ncc_value + 1.0) / 2.0;  // Convert [-1, 1] to [0, 1]
                    
                    if correlation >= threshold {
                        matches.push((x, y, correlation));
                    }
                }
            }
        }
    }
    
    eprintln!("100%");
    
    // Sort by correlation descending and limit results
    matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    matches.truncate(max_matches as usize);
    
    matches
}

#[derive(Default)]
struct MatchingStats {
    patches_loaded: u32,
    images_loaded: u32,
    total_comparisons: u32,
    matches_found: u32,
}

fn main() {
    let start = Instant::now();
    let test_images_dir = "assets/test_images";
    let threshold = 0.95; // 95% threshold for high-quality matches only
    let max_matches_per_patch = 1; // Only get top match
    let search_margin = 10u32; // Search within 10 pixels of expected location
    
    let mut stats = MatchingStats::default();
    
    println!("🔍 Template Matching Example");
    println!("{}", "=".repeat(70));
    
    // Load all patches
    println!("\n📦 Loading patches...");
    let load_start = Instant::now();
    let mut patches = Vec::new();
    
    if let Ok(entries) = fs::read_dir(test_images_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    let path = entry.path();
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy();
                        if filename_str.starts_with("patch-") && filename_str.ends_with(".png") {
                            let source_path = path.to_string_lossy().to_string();
                            let patch_load_start = Instant::now();
                            
                            if let Some((label, x, y, width, height)) = extract_region_and_label_from_filename(&source_path) {
                                match image::open(&source_path) {
                                    Ok(img) => {
                                        let patch_load_duration = patch_load_start.elapsed();
                                        let rgb = img.to_rgb8();
                                        patches.push((
                                            label.unwrap_or_else(|| "unlabeled".to_string()),
                                            filename_str.to_string(),
                                            rgb,
                                            x,
                                            y,
                                        ));
                                        stats.patches_loaded += 1;
                                        println!("  ✓ [{}] {} ({}x{}, orig: ({},{}), {:.2}ms elapsed)", 
                                            stats.patches_loaded, 
                                            filename_str, 
                                            img.width(), 
                                            img.height(),
                                            x, y,
                                            patch_load_duration.as_secs_f64() * 1000.0);
                                    }
                                    Err(e) => {
                                        eprintln!("  ✗ Failed to load patch {}: {}", filename_str, e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let load_duration = load_start.elapsed();
    println!("  ✅ Loaded {} patches in {:.2}ms total", stats.patches_loaded, load_duration.as_secs_f64() * 1000.0);
    
    // Load all images and run matching
    println!("\n🔎 Matching patches against images...");
    let match_start = Instant::now();
    let mut image_count = 0;
    
    if let Ok(entries) = fs::read_dir(test_images_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    let path = entry.path();
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy();
                        if filename_str.starts_with("img-") && filename_str.ends_with(".png") {
                            let source_path = path.to_string_lossy().to_string();
                            let image_start = Instant::now();
                            
                            match image::open(&source_path) {
                                Ok(img) => {
                                    let image_load_duration = image_start.elapsed();
                                    stats.images_loaded += 1;
                                    image_count += 1;
                                    println!("\n  📷 [{}/] Image: {} ({}x{}, loaded {:.2}ms)", 
                                        image_count,
                                        filename_str, 
                                        img.width(), 
                                        img.height(),
                                        image_load_duration.as_secs_f64() * 1000.0);
                                    
                                    for (patch_idx, (patch_label, patch_filename, patch_img, patch_orig_x, patch_orig_y)) in patches.iter().enumerate() {
                                        let search_start = Instant::now();
                                        stats.total_comparisons += 1;
                                        
                                        let region_desc = format!("x:[{},{}] y:[{},{}]",
                                            patch_orig_x.saturating_sub(search_margin),
                                            (patch_orig_x + patch_img.width() + search_margin),
                                            patch_orig_y.saturating_sub(search_margin),
                                            (patch_orig_y + patch_img.height() + search_margin));
                                        println!("      🔍 Patch {}/{} '{}' - searching region {} ...",
                                            patch_idx + 1, patches.len(), patch_label, region_desc);
                                        
                                        let matches = find_matches(&img, patch_img, threshold, max_matches_per_patch, 
                                            Some(*patch_orig_x), Some(*patch_orig_y), search_margin);
                                        let search_duration = search_start.elapsed();
                                        
                                        if !matches.is_empty() {
                                            println!("      ✓ Patch {}/'{}' ({}): found {} matches in {:.2}ms",
                                                patch_idx + 1,
                                                patch_label, 
                                                patch_filename,
                                                matches.len(),
                                                search_duration.as_secs_f64() * 1000.0);
                                            for (i, (x, y, correlation)) in matches.iter().enumerate() {
                                                println!("        [{}] Position: ({}, {}) - Correlation: {:.2}%",
                                                    i + 1, x, y, correlation * 100.0);
                                                stats.matches_found += 1;
                                            }
                                        } else {
                                            println!("      ✗ Patch {}/'{}' - No matches above {:.0}% ({:.2}ms)",
                                                patch_idx + 1,
                                                patch_label, 
                                                threshold * 100.0, 
                                                search_duration.as_secs_f64() * 1000.0);
                                        }
                                    }
                                    
                                    let image_total = image_start.elapsed();
                                    println!("    ⏱️  Image processing time: {:.2}ms", image_total.as_secs_f64() * 1000.0);
                                }
                                Err(e) => {
                                    eprintln!("  ✗ Failed to open image {}: {}", filename_str, e);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let match_duration = match_start.elapsed();
    
    let total_duration = start.elapsed();
    
    // Print summary
    println!("\n{}", "=".repeat(70));
    println!("📊 MATCHING SUMMARY");
    println!("{}", "=".repeat(70));
    println!("✓ Patches loaded:        {}", stats.patches_loaded);
    println!("✓ Images loaded:         {}", stats.images_loaded);
    println!("✓ Total comparisons:     {}", stats.total_comparisons);
    println!("✓ Matches found:         {}", stats.matches_found);
    println!("  Threshold:             {:.0}%", threshold * 100.0);
    println!("{}", "-".repeat(70));
    println!("⏱️  Load time:             {:.2}ms", load_duration.as_secs_f64() * 1000.0);
    println!("⏱️  Matching time:         {:.2}ms", match_duration.as_secs_f64() * 1000.0);
    println!("⏱️  Total time:            {:.2}ms ({:.3}s)", 
        total_duration.as_secs_f64() * 1000.0,
        total_duration.as_secs_f64());
    if stats.total_comparisons > 0 {
        println!("⏱️  Avg time per comparison: {:.2}ms",
            (match_duration.as_secs_f64() * 1000.0) / stats.total_comparisons as f64);
    }
    println!("{}", "=".repeat(70));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_region_and_label_from_patch_filename() {
        // Test with label
        let region = extract_region_and_label_from_filename("patch-button-[100,200,50,75].png");
        assert_eq!(region, Some((Some("button".to_string()), 100, 200, 50, 75)));
        
        // Test with compound label
        let region = extract_region_and_label_from_filename("patch-claim_button-[10,20,100,100].png");
        assert_eq!(region, Some((Some("claim_button".to_string()), 10, 20, 100, 100)));
        
        // Test with full path
        let region = extract_region_and_label_from_filename("assets/test_images/patch-dialog-[10,20,100,100].png");
        assert_eq!(region, Some((Some("dialog".to_string()), 10, 20, 100, 100)));
        
        // Test with spaces in coordinates
        let region = extract_region_and_label_from_filename("patch-label-[ 550 , 1345 , 500 , 200 ].png");
        assert_eq!(region, Some((Some("label".to_string()), 550, 1345, 500, 200)));
    }

    #[test]
    fn test_correlation_perfect_match() {
        // Create two identical images
        let img1 = image::RgbImage::from_fn(10, 10, |_, _| {
            image::Rgb([100u8, 150u8, 200u8])
        });
        let img2 = img1.clone();
        
        let correlation = calculate_correlation(&img1, &img2, 0.5);
        assert!((correlation - 1.0).abs() < 0.01, "Perfect match should have ~1.0 correlation");
    }

    #[test]
    fn test_correlation_different_images() {
        // Create two completely different images
        let img1 = image::RgbImage::from_fn(10, 10, |_, _| {
            image::Rgb([255u8, 255u8, 255u8])
        });
        let img2 = image::RgbImage::from_fn(10, 10, |_, _| {
            image::Rgb([0u8, 0u8, 0u8])
        });
        
        let correlation = calculate_correlation(&img1, &img2, 0.5);
        assert!(correlation < 0.1, "Completely different images should have low correlation");
    }

    #[test]
    fn test_correlation_size_mismatch() {
        let img1 = image::RgbImage::from_fn(10, 10, |_, _| {
            image::Rgb([100u8, 150u8, 200u8])
        });
        let img2 = image::RgbImage::from_fn(5, 5, |_, _| {
            image::Rgb([100u8, 150u8, 200u8])
        });
        
        let correlation = calculate_correlation(&img1, &img2, 0.5);
        assert_eq!(correlation, 0.0, "Size mismatch should return 0.0 correlation");
    }
}
