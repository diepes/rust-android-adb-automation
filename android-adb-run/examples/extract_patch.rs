//! Extract a region from a screenshot to create a template patch
//! Usage: cargo run --example extract_patch

#[allow(unused_imports)]
use image::GenericImageView;
use std::path::Path;

/// Generate a patch filename from coordinates and optional label
/// 
/// # Arguments
/// * `label` - Optional label to include in filename
/// * `x` - X coordinate of top-left corner
/// * `y` - Y coordinate of top-left corner
/// * `width` - Width of the region
/// * `height` - Height of the region
/// 
/// # Returns
/// A filename in format: `patch-[label-][x,y,width,height].png`
fn generate_patch_filename(label: Option<&str>, x: u32, y: u32, width: u32, height: u32) -> String {
    match label {
        Some(l) if !l.is_empty() => format!("patch-{}-[{},{},{},{}].png", l, x, y, width, height),
        _ => format!("patch-[{},{},{},{}].png", x, y, width, height),
    }
}

fn generate_output_path(source_path: &str, label: Option<&str>, x: u32, y: u32, width: u32, height: u32) -> String {
    let source = Path::new(source_path);
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let filename = generate_patch_filename(label, x, y, width, height);
    
    parent
        .join(&filename)
        .to_string_lossy()
        .to_string()
}

/// Extract region coordinates and label from source filename
/// 
/// Parses filenames in format: `img[-label]-[x,y,width,height].png`
/// Examples: 
/// - `img-[550,1345,500,200].png` → (None, 550, 1345, 500, 200)
/// - `img-button-[550,1345,500,200].png` → (Some("button"), 550, 1345, 500, 200)
/// - `img-claim_button-[300,1682,50,50].png` → (Some("claim_button"), 300, 1682, 50, 50)
/// 
/// # Arguments
/// * `source_path` - Path to the source file
/// 
/// # Returns
/// `Some((label, x, y, width, height))` if coordinates found in filename, `None` otherwise
fn extract_region_and_label_from_filename(source_path: &str) -> Option<(Option<String>, u32, u32, u32, u32)> {
    let path = Path::new(source_path);
    let filename = path.file_name()?.to_string_lossy();
    
    // Look for pattern: [x,y,width,height]
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
                    // Extract label if present
                    // Format: img-[optional-label-][x,y,w,h].png
                    let label = if bracket_start > 5 {  // "img-[" is 5 chars
                        let label_part = &filename[4..bracket_start].trim_end_matches('-');
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

fn main() {
    use std::fs;
    use std::time::Instant;
    
    let start = Instant::now();
    let test_images_dir = "assets/test_images";
    
    let mut stats = ProcessingStats {
        cleanup_count: 0,
        processed_count: 0,
        skipped_count: 0,
    };
    
    // First, remove all existing patch files (both patch-* and patch_* formats)
    println!("Cleaning up old patch files...");
    let cleanup_start = Instant::now();
    if let Ok(entries) = fs::read_dir(test_images_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    let path = entry.path();
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy();
                        if (filename_str.starts_with("patch-") || filename_str.starts_with("patch_")) && filename_str.ends_with(".png") {
                            if let Err(e) = fs::remove_file(&path) {
                                eprintln!("⚠️ Failed to remove {}: {}", filename_str, e);
                            } else {
                                println!("  Removed: {}", filename_str);
                                stats.cleanup_count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    let cleanup_duration = cleanup_start.elapsed();
    println!("  ✓ Cleanup took: {:.2}ms\n", cleanup_duration.as_secs_f64() * 1000.0);
    
    // Process all img-[x,y,width,height].png or img-label-[x,y,width,height].png files
    println!("Processing img-*.png files...");
    let process_start = Instant::now();
    if let Ok(entries) = fs::read_dir(test_images_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    let path = entry.path();
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy();
                        if filename_str.starts_with("img-") && filename_str.ends_with(".png") {
                            let source_path = path.to_string_lossy().to_string();
                            let file_start = Instant::now();
                            
                            // Extract region and label from filename
                            if let Some((label, x, y, width, height)) = extract_region_and_label_from_filename(&source_path) {
                                println!("\nProcessing: {}", filename_str);
                                if let Some(ref l) = label {
                                    println!("  Extracted label: {}", l);
                                }
                                println!("  Extracted region: x={}, y={}, width={}, height={}", x, y, width, height);
                                
                                // Generate output path
                                let output_path = generate_output_path(&source_path, label.as_deref(), x, y, width, height);
                                
                                // Load and crop image
                                let load_start = Instant::now();
                                match image::open(&source_path) {
                                    Ok(img) => {
                                        let load_duration = load_start.elapsed();
                                        println!("  Source dimensions: {}x{} (loaded in {:.2}ms)", 
                                            img.width(), img.height(), load_duration.as_secs_f64() * 1000.0);
                                        
                                        // Validate region is within image bounds
                                        if x + width > img.width() || y + height > img.height() {
                                            eprintln!("  ✗ Error: Region [{}..{}] × [{}..{}] exceeds image bounds [{}x{}]",
                                                x, x + width, y, y + height, img.width(), img.height());
                                            stats.skipped_count += 1;
                                        } else {
                                            // Crop the region
                                            let crop_start = Instant::now();
                                            let cropped = img.crop_imm(x, y, width, height);
                                            let crop_duration = crop_start.elapsed();
                                            
                                            // Save the patch
                                            let save_start = Instant::now();
                                            match cropped.save(&output_path) {
                                                Ok(_) => {
                                                    let save_duration = save_start.elapsed();
                                                    let total_file_duration = file_start.elapsed();
                                                    println!("  ✓ Saved patch to: {} (crop: {:.2}ms, save: {:.2}ms, total: {:.2}ms)", 
                                                        Path::new(&output_path).file_name().unwrap_or_default().to_string_lossy(),
                                                        crop_duration.as_secs_f64() * 1000.0,
                                                        save_duration.as_secs_f64() * 1000.0,
                                                        total_file_duration.as_secs_f64() * 1000.0);
                                                    stats.processed_count += 1;
                                                }
                                                Err(e) => {
                                                    eprintln!("  ✗ Failed to save patch: {}", e);
                                                    stats.skipped_count += 1;
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("  ✗ Failed to open source image: {}", e);
                                        stats.skipped_count += 1;
                                    }
                                }
                            } else {
                                eprintln!("⚠️ Could not extract region from: {}", filename_str);
                                eprintln!("   Expected format: img[-label]-[x,y,width,height].png");
                                stats.skipped_count += 1;
                            }
                        }
                    }
                }
            }
        }
    } else {
        eprintln!("✗ Failed to read directory: {}", test_images_dir);
    }
    let process_duration = process_start.elapsed();
    
    let total_duration = start.elapsed();
    
    // Print summary
    println!("\n{}", "=".repeat(60));
    println!("📊 PROCESSING SUMMARY");
    println!("{}", "=".repeat(60));
    println!("✓ Patches generated: {}", stats.processed_count);
    println!("⚠️ Patches skipped:  {}", stats.skipped_count);
    println!("🗑️  Old patches removed: {}", stats.cleanup_count);
    println!("{}", "-".repeat(60));
    println!("⏱️  Cleanup time:     {:.2}ms", cleanup_duration.as_secs_f64() * 1000.0);
    println!("⏱️  Processing time:  {:.2}ms", process_duration.as_secs_f64() * 1000.0);
    println!("⏱️  Total time:       {:.2}ms ({:.3}s)", 
        total_duration.as_secs_f64() * 1000.0,
        total_duration.as_secs_f64());
    println!("{}", "=".repeat(60));
}

struct ProcessingStats {
    cleanup_count: u32,
    processed_count: u32,
    skipped_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_patch_filename() {
        assert_eq!(
            generate_patch_filename(None, 100, 200, 50, 50),
            "patch-[100,200,50,50].png"
        );
        assert_eq!(
            generate_patch_filename(Some("button"), 100, 200, 50, 50),
            "patch-button-[100,200,50,50].png"
        );
        assert_eq!(
            generate_patch_filename(Some("claim_button"), 0, 0, 100, 100),
            "patch-claim_button-[0,0,100,100].png"
        );
    }

    #[test]
    fn test_generate_output_path() {
        let path = generate_output_path("assets/test_images/screenshot.png", None, 100, 200, 50, 50);
        assert_eq!(path, "assets/test_images/patch-[100,200,50,50].png");
        
        let path = generate_output_path("screenshot.png", Some("button"), 10, 20, 30, 40);
        assert_eq!(path, "patch-button-[10,20,30,40].png");
        
        let path = generate_output_path("assets/images/file.png", Some("claim"), 1, 2, 3, 4);
        assert_eq!(path, "assets/images/patch-claim-[1,2,3,4].png");
    }

    #[test]
    fn test_extract_region_and_label_from_filename() {
        // Test with no label
        let region = extract_region_and_label_from_filename("img-[550,1345,500,200].png");
        assert_eq!(region, Some((None, 550, 1345, 500, 200)));
        
        // Test with simple label
        let region = extract_region_and_label_from_filename("img-button-[100,200,50,75].png");
        assert_eq!(region, Some((Some("button".to_string()), 100, 200, 50, 75)));
        
        // Test with compound label
        let region = extract_region_and_label_from_filename("img-claim_button-[10,20,100,100].png");
        assert_eq!(region, Some((Some("claim_button".to_string()), 10, 20, 100, 100)));
        
        // Test with full path
        let region = extract_region_and_label_from_filename("assets/test_images/img-dialog-[10,20,100,100].png");
        assert_eq!(region, Some((Some("dialog".to_string()), 10, 20, 100, 100)));
        
        // Test with no coordinates
        let region = extract_region_and_label_from_filename("screenshot.png");
        assert_eq!(region, None);
        
        // Test with spaces in coordinates
        let region = extract_region_and_label_from_filename("img-label-[ 550 , 1345 , 500 , 200 ].png");
        assert_eq!(region, Some((Some("label".to_string()), 550, 1345, 500, 200)));
    }
}
