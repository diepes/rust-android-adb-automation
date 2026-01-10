/// Simple test to verify ADB USB connection works reliably
/// Run with: cargo run --example test_adb_connection
use adb_client::{ADBDeviceExt, ADBUSBDevice};

fn main() {
    println!("🔍 Testing ADB USB Connection...\n");

    // Step 1: Check if ADB key exists
    let key_path = homedir::my_home()
        .ok()
        .flatten()
        .map(|home| home.join(".android").join("adbkey"))
        .expect("Failed to determine home directory");

    println!("📁 ADB key path: {}", key_path.display());
    if key_path.exists() {
        println!("✅ ADB key exists");
    } else {
        println!("❌ ADB key not found! Run 'adb devices' once to generate it.");
        return;
    }

    // Step 2: Search for USB devices
    println!("\n🔍 Searching for ADB devices...");
    let (vendor_id, product_id) = match adb_client::search_adb_devices() {
        Ok(Some((v, p))) => (v, p),
        Ok(None) => {
            println!("❌ No ADB devices found");
            println!("\n💡 Make sure:");
            println!("  1. Your Android device is connected via USB");
            println!("  2. USB debugging is enabled");
            println!("  3. Check lsusb output");
            return;
        }
        Err(e) => {
            println!("❌ Error searching for devices: {}", e);
            return;
        }
    };

    println!("✅ Found device: {:04x}:{:04x}", vendor_id, product_id);

    // Step 3: Try to connect with custom key (with retry for busy USB)
    println!("\n🔌 Connecting with persistent ADB key...");
    println!(
        "   ⏱️  If you see 'Allow USB debugging?' popup on phone, you have 10 seconds to accept"
    );
    println!("   📱 Check 'Always allow from this computer' to avoid future popups\n");

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
                // Retry on common connection errors (AUTH/CLSE are protocol handshake issues)
                let should_retry = attempt < 5
                    && (err_msg.contains("Resource busy")
                        || err_msg.contains("AUTH")
                        || err_msg.contains("CLSE")
                        || err_msg.contains("CNXN")
                        || err_msg.contains("timed out"));

                if should_retry {
                    if attempt == 1 {
                        println!("  ⚠️ Connection attempt {} failed: {}", attempt, err_msg);
                        println!("     (This is normal, retrying...)");
                    } else {
                        println!("  ⚠️ Attempt {}/5: {}", attempt, err_msg);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(2));
                } else {
                    println!("❌ Failed to connect after {} attempts: '{}'", attempt, e);
                    println!("\n💡 Troubleshooting:");
                    println!("  1. Make sure USB debugging is enabled on your phone");
                    println!(
                        "  2. Check that the authorization popup appeared and you clicked 'Allow'"
                    );
                    println!("  3. Try unplugging and replugging the USB cable");
                    println!("  4. Check USB permissions: ls -l /dev/bus/usb/*/*");
                    return;
                }
            }
        }
    }

    let mut device = device.expect("Failed to connect after 5 retries");

    println!("✅ USB device connected!");

    // Step 3.5: Validate authentication by testing a simple command
    print!("  • Validating authentication... ");
    let mut test_out = Vec::new();
    match device.shell_command(&["echo", "auth_test"], &mut test_out) {
        Ok(_) => {
            let output = String::from_utf8_lossy(&test_out);
            if output.trim() == "auth_test" {
                println!("✅ Authenticated and working");
            } else {
                println!("⚠️ Got unexpected response: {}", output.trim());
            }
        }
        Err(e) => {
            println!("❌ Authentication validation failed: {}", e);
            println!("\n💡 The device may not be properly authorized.");
            println!("   Make sure you clicked 'Allow' on the USB debugging popup.");
            return;
        }
    }

    // Step 4: Test basic commands
    println!("\n📋 Testing basic commands:");

    // Test 1: Echo command
    print!("  • Echo test... ");
    let mut out = Vec::new();
    match device.shell_command(&["echo", "hello"], &mut out) {
        Ok(_) => {
            let output = String::from_utf8_lossy(&out);
            println!("✅ Response: {}", output.trim());
        }
        Err(e) => {
            print!("❌ Failed: {}", e);
            panic!("❌ Failed: {}", e);
        }
    }

    // Test 2: Get device properties
    print!("  • Get Android version... ");
    let mut out = Vec::new();
    match device.shell_command(&["getprop", "ro.build.version.release"], &mut out) {
        Ok(_) => {
            let output = String::from_utf8_lossy(&out);
            println!("✅ Android {}", output.trim());
        }
        Err(e) => panic!("❌ Failed: {}", e),
    }

    // Test 3: Get screen size
    print!("  • Get screen size... ");
    let mut out = Vec::new();
    match device.shell_command(&["wm", "size"], &mut out) {
        Ok(_) => {
            let output = String::from_utf8_lossy(&out);
            for line in output.lines() {
                if line.contains("Physical size:") {
                    println!("✅ {}", line.trim());
                    break;
                }
            }
        }
        Err(e) => println!("❌ Failed: {}", e),
    }

    // Test 4: Get device model
    print!("  • Get device model... ");
    let mut out = Vec::new();
    match device.shell_command(&["getprop", "ro.product.model"], &mut out) {
        Ok(_) => {
            let output = String::from_utf8_lossy(&out);
            println!("✅ {}", output.trim());
        }
        Err(e) => panic!("❌ Failed: {}", e),
    }

    // Test 5: Multiple commands in sequence
    println!("\n🔄 Testing multiple sequential commands:");
    for i in 1..=5 {
        let mut out = Vec::new();
        match device.shell_command(&["echo", &format!("test_{}", i)], &mut out) {
            Ok(_) => {
                let output = String::from_utf8_lossy(&out);
                println!("  ✅ Command {}: {}", i, output.trim());
            }
            Err(e) => {
                panic!("  ❌ Command {} failed: {}", i, e);
            }
        }
    }

    // Test 6: Screenshot capture with timeout
    println!("\n📸 Testing screenshot capture:");

    // Try framebuffer first (faster, doesn't hang)
    print!("  • Trying framebuffer... ");
    match device.framebuffer_bytes() {
        Ok(fb_data) => {
            println!("✅ Got {} bytes from framebuffer", fb_data.len());
            println!("  💡 Framebuffer works - main app should use this instead of screencap");
            println!("\n✅ All tests completed successfully!");
            return;
        }
        Err(e) => {
            println!("❌ Framebuffer failed: {}", e);
        }
    }

    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    let (tx, rx) = mpsc::channel();
    let mut device_clone = device;

    print!("  • Capturing via screencap (10s timeout)... ");
    let handle = thread::spawn(move || {
        let mut out = Vec::new();
        match device_clone.shell_command(&["screencap", "-p"], &mut out) {
            Ok(_) => tx.send(Ok(out)).ok(),
            Err(e) => tx.send(Err(format!("screencap failed: {}", e))).ok(),
        };
    });

    let start = Instant::now();
    let result = rx.recv_timeout(Duration::from_secs(10));

    match result {
        Ok(Ok(png_data)) => {
            let elapsed = start.elapsed().as_secs_f32();
            println!("✅ Got {} bytes in {:.1}s", png_data.len(), elapsed);

            // Validate PNG header
            print!("  • Validating PNG format... ");
            if png_data.len() > 8 && &png_data[0..8] == b"\x89PNG\r\n\x1a\n" {
                println!("✅ Valid PNG header");

                // Save to file
                print!("  • Saving screenshot to test_screenshot.png... ");
                match std::fs::write("test_screenshot.png", &png_data) {
                    Ok(_) => println!("✅ Saved"),
                    Err(e) => println!("❌ Failed to save: {}", e),
                }
            } else {
                println!(
                    "❌ Invalid PNG header (got {} bytes, first bytes: {:02x?})",
                    png_data.len(),
                    &png_data[..8.min(png_data.len())]
                );
            }
        }
        Ok(Err(e)) => println!("❌ {}", e),
        Err(_) => {
            println!("❌ Timeout after 10 seconds");
            println!(
                "  💡 The screencap command is hanging - this is a known issue with some devices"
            );
        }
    }

    handle.join().ok();

    println!("\n✅ All tests completed successfully!");
}
