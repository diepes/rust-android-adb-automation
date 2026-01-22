//! Tests for image matching functionality

use crate::game_automation::match_image::{
    DetectionResult, MatchConfig, SearchRegion, Template, TemplateCategory, TemplateMatch,
};
use std::path::Path;

/// Test assets directory path
const TEST_IMAGES_DIR: &str = "assets/test_images";

/// Helper to check if test assets exist
fn test_assets_available() -> bool {
    Path::new(TEST_IMAGES_DIR).exists()
}

#[test]
fn test_region_parse_from_filename() {
    // Test parsing region coordinates from filename
    let region = SearchRegion::parse_from_filename("img-[300,1682,50,50].png", 1080, 2280);

    assert_eq!(region.x, 300);
    assert_eq!(region.y, 1682);
    assert_eq!(region.width, 50);
    assert_eq!(region.height, 50);
}

#[test]
fn test_region_parse_full_screen_fallback() {
    // When no region in filename, should return full screen
    let region = SearchRegion::parse_from_filename("button_start.png", 1080, 2280);

    assert_eq!(region.x, 0);
    assert_eq!(region.y, 0);
    assert_eq!(region.width, 1080);
    assert_eq!(region.height, 2280);
}

#[test]
fn test_region_clips_to_screen_bounds() {
    // Region that exceeds screen bounds should be clipped
    let region = SearchRegion::parse_from_filename("img-[1000,2200,200,200].png", 1080, 2280);

    // Should be clipped: x=1000, width should be min(200, 1080-1000) = 80
    assert_eq!(region.x, 1000);
    assert_eq!(region.width, 80);
    // y=2200, height should be min(200, 2280-2200) = 80
    assert_eq!(region.y, 2200);
    assert_eq!(region.height, 80);
}

#[test]
fn test_match_config_defaults() {
    let config = MatchConfig::default();

    assert_eq!(config.confidence_threshold, 0.8);
    assert_eq!(config.max_matches_per_template, 1);
    assert!(!config.enable_multiscale);
    assert!(!config.debug_enabled);
}

#[test]
fn test_detection_result_has_matches() {
    let mut result = DetectionResult::new();
    assert!(!result.has_matches());

    // Add a dummy match
    let region = SearchRegion::new(0, 0, 100, 100, "test".to_string());
    let template = Template {
        path: "test.png".to_string(),
        name: "test".to_string(),
        search_region: region,
        width: 50,
        height: 50,
        category: TemplateCategory::Unknown,
    };
    let template_match = TemplateMatch::new(template, 10, 10, 0.95, 1.0);
    result.matches.push(template_match);

    assert!(result.has_matches());
}

#[test]
fn test_detection_result_best_match() {
    let mut result = DetectionResult::new();

    let region = SearchRegion::new(0, 0, 100, 100, "test".to_string());

    // Add matches with different confidence
    for (i, conf) in [0.85, 0.95, 0.90].iter().enumerate() {
        let template = Template {
            path: format!("test{i}.png"),
            name: format!("test{i}"),
            search_region: region.clone(),
            width: 50,
            height: 50,
            category: TemplateCategory::Unknown,
        };
        result
            .matches
            .push(TemplateMatch::new(template, 10, 10, *conf, 1.0));
    }

    let best = result.best_match().unwrap();
    assert_eq!(best.confidence, 0.95);
    assert_eq!(best.template.name, "test1");
}

#[test]
fn test_template_match_tap_coordinates() {
    let region = SearchRegion::new(0, 0, 200, 200, "test".to_string());
    let template = Template {
        path: "test.png".to_string(),
        name: "test".to_string(),
        search_region: region,
        width: 50,
        height: 50,
        category: TemplateCategory::Unknown,
    };

    // Match at position (100, 150), template is 50x50
    // Center should be (100 + 25, 150 + 25) = (125, 175)
    let template_match = TemplateMatch::new(template, 100, 150, 0.95, 1.0);
    let (tap_x, tap_y) = template_match.get_tap_coordinates();

    assert_eq!(tap_x, 125);
    assert_eq!(tap_y, 175);
}

#[test]
fn test_template_match_within_bounds() {
    let region = SearchRegion::new(0, 0, 200, 200, "test".to_string());
    let template = Template {
        path: "test.png".to_string(),
        name: "test".to_string(),
        search_region: region,
        width: 50,
        height: 50,
        category: TemplateCategory::Unknown,
    };

    // Match within bounds
    let template_match = TemplateMatch::new(template.clone(), 100, 150, 0.95, 1.0);
    assert!(template_match.is_within_bounds(1080, 2280));

    // Match outside bounds (tap would be at x=1050+25=1075, y=2250+25=2275)
    // That's still within 1080x2280
    let template_match2 = TemplateMatch::new(template.clone(), 1050, 2250, 0.95, 1.0);
    assert!(template_match2.is_within_bounds(1080, 2280));

    // But if template is at 1060, tap_x = 1060+25 = 1085 > 1080, outside
    let template_match3 = TemplateMatch::new(template, 1060, 100, 0.95, 1.0);
    assert!(!template_match3.is_within_bounds(1080, 2280));
}

// Integration tests that require actual image files
#[test]
fn test_match_patch_against_source_screenshot() {
    if !test_assets_available() {
        eprintln!("Skipping test: test assets not available");
        return;
    }

    let patch_path = format!("{}/patch_300_1682_50x50.png", TEST_IMAGES_DIR);
    let source_path = format!("{}/img-[300,1682,50,50].png", TEST_IMAGES_DIR);

    if !Path::new(&patch_path).exists() || !Path::new(&source_path).exists() {
        eprintln!("Skipping test: required image files not found");
        return;
    }

    // Load the source screenshot
    let source_img = image::open(&source_path).expect("Failed to load source");
    let source_gray = source_img.to_luma8();

    // Load the patch
    let patch_img = image::open(&patch_path).expect("Failed to load patch");
    let patch_gray = patch_img.to_luma8();

    // The patch was extracted from (300, 1682), so matching should find it there
    // Create a search region around that area
    let search_region = image::imageops::crop_imm(
        &source_gray,
        250,  // x - 50 pixels before
        1632, // y - 50 pixels before
        150,  // width = 50 + 50 + 50
        150,  // height = 50 + 50 + 50
    );
    let search_buffer = search_region.to_image();

    // Perform template matching
    use imageproc::template_matching::{MatchTemplateMethod, match_template};
    let result = match_template(
        &search_buffer,
        &patch_gray,
        MatchTemplateMethod::CrossCorrelationNormalized,
    );

    // Find the best match
    // Note: CrossCorrelationNormalized returns f32 values in range [-1, 1]
    // where 1.0 is a perfect match
    let mut max_confidence = f32::MIN;
    let mut best_pos = (0u32, 0u32);

    for (x, y, pixel) in result.enumerate_pixels() {
        let confidence = pixel[0]; // Already f32 in range [-1, 1]
        if confidence > max_confidence {
            max_confidence = confidence;
            best_pos = (x, y);
        }
    }

    println!(
        "Best match: confidence={:.4} at ({}, {})",
        max_confidence, best_pos.0, best_pos.1
    );

    // The patch was extracted from (300, 1682) in the original
    // We're searching in a region starting at (250, 1632)
    // So the expected match should be around (50, 50) in the cropped region
    // (300 - 250 = 50, 1682 - 1632 = 50)

    // Since this is the same image, confidence should be very high (>0.99)
    assert!(
        max_confidence > 0.99,
        "Expected near-perfect match, got {:.4}",
        max_confidence
    );

    // Position should be around (50, 50) in the search region
    assert!(
        best_pos.0 >= 45 && best_pos.0 <= 55,
        "Expected x around 50, got {}",
        best_pos.0
    );
    assert!(
        best_pos.1 >= 45 && best_pos.1 <= 55,
        "Expected y around 50, got {}",
        best_pos.1
    );
}

#[test]
fn test_match_patch_against_different_screenshot() {
    if !test_assets_available() {
        eprintln!("Skipping test: test assets not available");
        return;
    }

    let patch_path = format!("{}/patch_300_1682_50x50.png", TEST_IMAGES_DIR);
    let other_screenshot = format!("{}/screenshot_1762059587.png", TEST_IMAGES_DIR);

    if !Path::new(&patch_path).exists() || !Path::new(&other_screenshot).exists() {
        eprintln!("Skipping test: required image files not found");
        return;
    }

    // Load the other screenshot
    let source_img = image::open(&other_screenshot).expect("Failed to load other screenshot");
    let source_gray = source_img.to_luma8();

    // Load the patch
    let patch_img = image::open(&patch_path).expect("Failed to load patch");
    let patch_gray = patch_img.to_luma8();

    // Search in the same region where the patch was extracted from the original
    let search_region = image::imageops::crop_imm(&source_gray, 250, 1632, 150, 150);
    let search_buffer = search_region.to_image();

    use imageproc::template_matching::{MatchTemplateMethod, match_template};
    let result = match_template(
        &search_buffer,
        &patch_gray,
        MatchTemplateMethod::CrossCorrelationNormalized,
    );

    // Find the best match
    // Note: CrossCorrelationNormalized returns f32 values in range [-1, 1]
    let mut max_confidence = f32::MIN;
    for (_x, _y, pixel) in result.enumerate_pixels() {
        let confidence = pixel[0]; // Already f32
        if confidence > max_confidence {
            max_confidence = confidence;
        }
    }

    println!(
        "Match confidence on different screenshot: {:.4}",
        max_confidence
    );

    // This tests that we can measure match quality
    // The patch may or may not match the other screenshot depending on content
    // We just verify the matching runs without errors and returns a valid confidence
    // CrossCorrelationNormalized should be in [-1, 1] range
    assert!((-1.0..=1.0).contains(&max_confidence));
}
