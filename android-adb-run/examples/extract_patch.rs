//! Extract a region from a screenshot to create a template patch
//! Usage: cargo run --example extract_patch

use image::GenericImageView;

fn main() {
    let source_path = "img-[300,1682,50,50].png";
    let output_path = "assets/test_images/patch_300_1682_50x50.png";
    
    // Region to extract
    let x = 300u32;
    let y = 1682u32;
    let width = 50u32;
    let height = 50u32;
    
    println!("Loading source image: {}", source_path);
    let img = image::open(source_path).expect("Failed to open source image");
    println!("Source dimensions: {}x{}", img.width(), img.height());
    
    // Crop the region
    let cropped = img.crop_imm(x, y, width, height);
    println!("Cropped region: {}x{} at ({}, {})", width, height, x, y);
    
    // Save the patch
    cropped.save(output_path).expect("Failed to save patch");
    println!("Saved patch to: {}", output_path);
}
