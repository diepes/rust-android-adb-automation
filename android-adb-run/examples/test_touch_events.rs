/// Test touch event detection with getevent
/// Run with: cargo run --example test_touch_events
use adb_client::{ADBDeviceExt, ADBUSBDevice};

fn main() {
    println!("🔍 Testing Touch Event Detection\n");

    // Check if ADB key exists
    let key_path = homedir::my_home()
        .ok()
        .flatten()
        .map(|home| home.join(".android").join("adbkey"))
        .expect("Failed to determine home directory");

    if !key_path.exists() {
        println!("❌ ADB key not found! Run 'adb devices' once to generate it.");
        return;
    }

    // Connect to device
    println!("🔌 Connecting to USB device...");
    let mut device = match ADBUSBDevice::autodetect_with_custom_private_key(key_path) {
        Ok(dev) => dev,
        Err(e) => {
            println!("❌ Failed to connect: {}", e);
            return;
        }
    };
    println!("✅ Connected\n");

    // Test 1: Find touch device with getevent -p
    println!("📱 Test 1: Finding touch input device...");
    let mut out = Vec::new();
    match device.shell_command(&["getevent", "-p"], &mut out) {
        Ok(_) => {
            let output = String::from_utf8_lossy(&out);
            println!("Output from getevent -p:");
            for line in output.lines().take(50) {
                if line.contains("/dev/input/event")
                    || line.contains("name:")
                    || line.contains("ABS")
                {
                    println!("  {}", line);
                }
            }
        }
        Err(e) => println!("❌ getevent -p failed: {}", e),
    }

    println!("\n📱 Test 2: Testing getevent with timeout...");
    println!("   Please TAP the screen NOW (you have 3 seconds)...\n");

    let mut out = Vec::new();
    let command = "timeout 3s getevent -lt /dev/input/event2 2>/dev/null || true";
    match device.shell_command(&["sh", "-c", command], &mut out) {
        Ok(_) => {
            let output = String::from_utf8_lossy(&out);
            if output.trim().is_empty() {
                println!("   ⚠️  No output from getevent (command may have failed)");
            } else {
                println!("   ✅ Received {} bytes of output:", out.len());
                for line in output.lines().take(20) {
                    println!("     {}", line);
                }
                if output.lines().count() > 20 {
                    println!("     ... ({} more lines)", output.lines().count() - 20);
                }
            }
        }
        Err(e) => println!("   ❌ Command failed: {}", e),
    }

    println!("\n📱 Test 3: Testing getevent WITHOUT timeout...");
    println!("   Please TAP the screen NOW (you have 2 seconds)...\n");

    let mut _out: Vec<u8> = Vec::new();
    // Use a background thread to run getevent, with manual timeout
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let key_path = homedir::my_home()
            .unwrap()
            .unwrap()
            .join(".android")
            .join("adbkey");
        let mut dev = ADBUSBDevice::autodetect_with_custom_private_key(key_path).unwrap();
        let mut out = Vec::new();

        // Start getevent in background
        let _ = dev.shell_command(&["getevent", "-lt", "/dev/input/event2"], &mut out);
        tx.send(out).ok();
    });

    // Wait for 2 seconds
    match rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output);
            if text.trim().is_empty() {
                println!("   ⚠️  No output received");
            } else {
                println!("   ✅ Received {} bytes:", output.len());
                for line in text.lines().take(20) {
                    println!("     {}", line);
                }
            }
        }
        Err(_) => {
            println!("   ⏱️  Timeout (command is still running - this means getevent is blocking)")
        }
    }

    println!("\n📱 Test 4: Testing with -c 10 (count limit)...");
    println!("   Please TAP the screen NOW...\n");

    let mut out = Vec::new();
    match device.shell_command(
        &["getevent", "-lt", "-c", "10", "/dev/input/event2"],
        &mut out,
    ) {
        Ok(_) => {
            let output = String::from_utf8_lossy(&out);
            if output.trim().is_empty() {
                println!("   ⚠️  No output");
            } else {
                println!("   ✅ Received {} events:", output.lines().count());
                for line in output.lines() {
                    println!("     {}", line);
                }
            }
        }
        Err(e) => println!("   ❌ Command failed: {}", e),
    }

    println!("\n✅ Touch event tests complete!");
}
