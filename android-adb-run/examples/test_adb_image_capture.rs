/// Test ADB image/screenshot capture methods
/// Run with: cargo run --example test_adb_image_capture
use adb_client::{ADBDeviceExt, ADBUSBDevice};
use std::fs;

fn main() {
    println!("🔍 Testing ADB Image Capture Methods...\n");

    // Step 1: Check if ADB key exists
    let key_path = homedir::my_home()
        .ok()
        .flatten()
        .map(|home| home.join(".android").join("adbkey"))
        .expect("Failed to determine home directory");

    println!("📁 ADB key path: {}", key_path.display());
    if !key_path.exists() {
        println!("❌ ADB key not found! Run 'adb devices' once to generate it.");
        return;
    }
    println!("✅ ADB key exists\n");

    // Step 2: Search for USB devices
    println!("🔍 Searching for ADB devices...");
    let (_vendor_id, _product_id) = match adb_client::search_adb_devices() {
        Ok(Some((v, p))) => {
            println!("✅ Found device: {:04x}:{:04x}\n", v, p);
            (v, p)
        }
        Ok(None) => {
            println!("❌ No ADB devices found");
            return;
        }
        Err(e) => {
            println!("❌ Error searching for devices: {}", e);
            return;
        }
    };

    // Step 3: Connect with retry logic
    println!("🔌 Connecting with persistent ADB key...");
    println!("   ⏱️  If you see 'Allow USB debugging?' popup, you have 10 seconds to accept\n");

    let mut device = None;
    for attempt in 1..=5 {
        match ADBUSBDevice::autodetect_with_custom_private_key(key_path.clone()) {
            Ok(dev) => {
                device = Some(dev);
                if attempt > 1 {
                    println!("  ✅ Connected on attempt {}", attempt);
                }
                break;
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                let should_retry = attempt < 5
                    && (err_msg.contains("Resource busy")
                        || err_msg.contains("AUTH")
                        || err_msg.contains("CLSE")
                        || err_msg.contains("CNXN"));

                if should_retry {
                    if attempt == 1 {
                        println!("  ⚠️  Connection attempt {} failed: {}", attempt, err_msg);
                        println!("     (This is normal for first connection, retrying...)");
                    } else {
                        println!("  ⚠️  Attempt {}/5: {}", attempt, err_msg);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(2));
                } else {
                    println!("❌ Failed to connect after {} attempts: {}", attempt, e);
                    return;
                }
            }
        }
    }

    let mut device = device.expect("Failed to connect after 5 retries");
    println!("✅ USB device connected!\n");

    // Step 4: Validate authentication
    print!("🔐 Validating authentication... ");
    let mut test_out = Vec::new();
    match device.shell_command(&["echo", "auth_test"], &mut test_out) {
        Ok(_) => {
            let output = String::from_utf8_lossy(&test_out);
            if output.trim() == "auth_test" {
                println!("✅ Authenticated\n");
            } else {
                println!("⚠️  Got unexpected response: {}", output.trim());
            }
        }
        Err(e) => {
            println!("❌ Authentication failed: {}", e);
            return;
        }
    }

    // Step 5: Test different image capture methods
    println!("📸 Testing Image Capture Methods:\n");

    // Method 1: Framebuffer
    println!("1️⃣  Testing framebuffer_bytes()...");
    match device.framebuffer_bytes() {
        Ok(fb_data) => {
            println!("   ✅ Framebuffer captured: {} bytes", fb_data.len());

            // Detect format by checking magic bytes
            let format_detected = if fb_data.len() >= 8 && &fb_data[0..8] == b"\x89PNG\r\n\x1a\n" {
                println!("   🎨 Format detected: PNG (magic bytes verified)");
                "PNG"
            } else if fb_data.len() >= 3
                && fb_data[0] == 0xFF
                && fb_data[1] == 0xD8
                && fb_data[2] == 0xFF
            {
                println!("   🎨 Format detected: JPEG");
                "JPEG"
            } else {
                println!("   🎨 Format: Unknown/Raw (no PNG or JPEG header)");
                "RAW"
            };

            // Get screen size to calculate bytes per pixel
            let mut out = Vec::new();
            if device.shell_command(&["wm", "size"], &mut out).is_ok() {
                let output = String::from_utf8_lossy(&out);
                for line in output.lines() {
                    if let Some(size_str) = line.strip_prefix("Physical size: ") {
                        let parts: Vec<&str> = size_str.trim().split('x').collect();
                        if parts.len() == 2
                            && let (Ok(width), Ok(height)) =
                                (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                        {
                            let pixels = (width * height) as usize;
                            let bpp = fb_data.len() as f64 / pixels as f64;
                            println!(
                                "   📊 Screen: {}x{} ({} pixels), {:.2} bytes/pixel",
                                width, height, pixels, bpp
                            );

                            if format_detected == "RAW" {
                                if bpp < 1.0 {
                                    println!("   ⚠️  Compressed/encoded format (< 1 byte/pixel)");
                                } else if (1.9..2.1).contains(&bpp) {
                                    println!("   💡 Likely RGB565 format (2 bytes/pixel)");
                                } else if (2.9..3.1).contains(&bpp) {
                                    println!("   💡 Likely RGB/BGR format (3 bytes/pixel)");
                                } else if (3.9..4.1).contains(&bpp) {
                                    println!("   💡 Likely RGBA/BGRA format (4 bytes/pixel)");
                                }
                            }
                        }
                    }
                }
            }

            // Save with appropriate extension
            let filename = match format_detected {
                "PNG" => "test_framebuffer.png",
                "JPEG" => "test_framebuffer.jpg",
                _ => "test_framebuffer.raw",
            };

            match fs::write(filename, &fb_data) {
                Ok(_) => println!("   💾 Saved to: {}", filename),
                Err(e) => println!("   ⚠️  Could not save: {}", e),
            }

            // If PNG, try to verify it can be loaded
            if format_detected == "PNG" {
                match image::load_from_memory(&fb_data) {
                    Ok(img) => {
                        println!(
                            "   ✅ PNG successfully decoded: {}x{}",
                            img.width(),
                            img.height()
                        );
                    }
                    Err(e) => {
                        println!("   ⚠️  PNG decode failed: {}", e);
                    }
                }
            }
        }
        Err(e) => println!("   ❌ Framebuffer failed: {}", e),
    }

    println!();

    // Method 2: Screencap PNG (may hang on some devices)
    println!("2️⃣  Testing screencap -p (PNG output with 10s timeout)...");
    println!("   ⚠️  WARNING: This may hang indefinitely on some devices!");

    let device_clone = std::sync::Arc::new(std::sync::Mutex::new(device));
    let device_for_thread = device_clone.clone();

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut dev = device_for_thread.lock().unwrap();
        let mut out = Vec::new();
        match dev.shell_command(&["screencap", "-p"], &mut out) {
            Ok(_) => tx.send(Ok(out)).ok(),
            Err(e) => tx.send(Err(format!("{}", e))).ok(),
        };
    });

    let start = std::time::Instant::now();
    match rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(Ok(png_data)) => {
            let elapsed = start.elapsed().as_secs_f32();
            println!(
                "   ✅ Screencap captured: {} bytes in {:.1}s",
                png_data.len(),
                elapsed
            );

            // Validate PNG header
            if png_data.len() > 8 && &png_data[0..8] == b"\x89PNG\r\n\x1a\n" {
                println!("   ✅ Valid PNG format");
                match fs::write("test_screencap.png", &png_data) {
                    Ok(_) => {
                        println!("   💾 Saved to: test_screencap.png");
                        // Try to decode it
                        match image::load_from_memory(&png_data) {
                            Ok(img) => {
                                println!(
                                    "   ✅ PNG successfully decoded: {}x{}",
                                    img.width(),
                                    img.height()
                                );
                            }
                            Err(e) => {
                                println!("   ⚠️  PNG decode failed: {}", e);
                            }
                        }
                    }
                    Err(e) => println!("   ⚠️  Could not save: {}", e),
                }
            } else {
                println!("   ❌ Invalid PNG header");
            }
        }
        Ok(Err(e)) => println!("   ❌ Screencap failed: {}", e),
        Err(_) => {
            println!("   ❌ Screencap timed out after 10 seconds (device may not support it)");
            println!("   💡 This is a known issue on some devices - screencap hangs indefinitely");
        }
    }

    println!();

    // Method 3: Screencap JPG (may be faster)
    println!("3️⃣  Testing screencap -j (JPEG output with 10s timeout)...");

    let device_for_thread2 = device_clone.clone();
    let (tx2, rx2) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut dev = device_for_thread2.lock().unwrap();
        let mut out = Vec::new();
        match dev.shell_command(&["screencap", "-j"], &mut out) {
            Ok(_) => tx2.send(Ok(out)).ok(),
            Err(e) => tx2.send(Err(format!("{}", e))).ok(),
        };
    });

    let start = std::time::Instant::now();
    match rx2.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(Ok(jpg_data)) => {
            let elapsed = start.elapsed().as_secs_f32();
            println!(
                "   ✅ Screencap JPEG captured: {} bytes in {:.1}s",
                jpg_data.len(),
                elapsed
            );

            // Validate JPEG header (FF D8 FF)
            if jpg_data.len() > 3 && jpg_data[0] == 0xFF && jpg_data[1] == 0xD8 {
                println!("   ✅ Valid JPEG format");
                match fs::write("test_screencap.jpg", &jpg_data) {
                    Ok(_) => {
                        println!("   💾 Saved to: test_screencap.jpg");
                        // Try to decode it
                        match image::load_from_memory(&jpg_data) {
                            Ok(img) => {
                                println!(
                                    "   ✅ JPEG successfully decoded: {}x{}",
                                    img.width(),
                                    img.height()
                                );
                            }
                            Err(e) => {
                                println!("   ⚠️  JPEG decode failed: {}", e);
                            }
                        }
                    }
                    Err(e) => println!("   ⚠️  Could not save: {}", e),
                }
            } else {
                println!("   ❌ Invalid JPEG header");
            }
        }
        Ok(Err(e)) => println!("   ❌ Screencap JPEG failed: {}", e),
        Err(_) => {
            println!("   ❌ Screencap JPEG timed out after 10 seconds");
        }
    }

    println!("\n📋 Summary:");
    println!("  • Framebuffer: Fast, but may use compressed/unusual formats");
    println!("  • Screencap PNG: High quality, but may hang on some devices");
    println!("  • Screencap JPEG: Smaller size, but still may hang");
    println!("\n💡 Recommendation: Use framebuffer if available, implement proper decoders");
    println!("   for compressed formats, or accept placeholder images when capture fails.");

    println!("\n✅ All tests completed!");
}
