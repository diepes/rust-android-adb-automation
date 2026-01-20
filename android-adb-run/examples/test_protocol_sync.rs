/// Diagnostic test to investigate ADB protocol synchronization issues
///
/// This test aims to reproduce and diagnose the "wrong command CLSE" errors
/// that occur when the ADB protocol gets out of sync.
///
/// Run with: cargo run --example test_protocol_sync
use adb_client::{ADBDeviceExt, ADBUSBDevice};
use std::time::{Duration, Instant};

fn main() {
    println!("🔍 ADB Protocol Synchronization Test");
    println!("=====================================\n");

    // Step 1: Setup
    let key_path = homedir::my_home()
        .ok()
        .flatten()
        .map(|home| home.join(".android").join("adbkey"))
        .expect("Failed to determine home directory");

    if !key_path.exists() {
        println!("❌ ADB key not found! Run 'adb devices' once to generate it.");
        return;
    }

    // Step 2: Connect with retries
    println!("🔌 Connecting to device...");
    let mut device = match connect_with_retry(&key_path, 5) {
        Some(d) => d,
        None => {
            println!("❌ Failed to connect after 5 attempts");
            return;
        }
    };
    println!("✅ Connected!\n");

    // Test 1: Rapid sequential shell commands (tests CLSE consumption)
    println!("📋 Test 1: Rapid Sequential Commands");
    println!("   This tests if CLSE messages are properly consumed between commands.");
    test_rapid_sequential_commands(&mut device);

    // Test 2: Mixed command types
    println!("\n📋 Test 2: Mixed Command Types (tap + screenshot simulation)");
    test_mixed_commands(&mut device);

    // Test 3: Stress test with fast tap commands
    println!("\n📋 Test 3: Rapid Tap Commands (stress test)");
    test_rapid_taps(&mut device);

    // Test 4: Long running command followed by short
    println!("\n📋 Test 4: Long Command Followed by Short");
    test_long_then_short(&mut device);

    println!("\n✅ All tests completed!");
}

/// Helper to run a shell command with multiple arguments
fn run_shell_cmd(device: &mut ADBUSBDevice, args: &[&str]) -> Result<String, String> {
    let mut out = Vec::new();
    match device.shell_command(args, &mut out) {
        Ok(_) => Ok(String::from_utf8_lossy(&out).to_string()),
        Err(e) => Err(format!("{}", e)),
    }
}

fn connect_with_retry(key_path: &std::path::Path, max_attempts: u32) -> Option<ADBUSBDevice> {
    for attempt in 1..=max_attempts {
        match ADBUSBDevice::autodetect_with_custom_private_key(key_path.to_path_buf()) {
            Ok(dev) => {
                if attempt > 1 {
                    println!("  ✅ Connected on attempt {}", attempt);
                }
                return Some(dev);
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                println!("  ⚠️ Attempt {}/{}: {}", attempt, max_attempts, err_msg);
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
    None
}

fn test_rapid_sequential_commands(device: &mut ADBUSBDevice) {
    let test_values = vec!["test1", "test2", "test3", "test4", "test5"];

    let mut success_count = 0;
    let mut fail_count = 0;
    let mut last_error = String::new();

    for (i, val) in test_values.iter().enumerate() {
        let start = Instant::now();

        match run_shell_cmd(device, &["echo", val]) {
            Ok(output) => {
                println!(
                    "  ✅ Command {}: 'echo {}' -> {} ({:?})",
                    i + 1,
                    val,
                    output.trim(),
                    start.elapsed()
                );
                success_count += 1;
            }
            Err(err) => {
                println!(
                    "  ❌ Command {}: 'echo {}' -> FAILED: {} ({:?})",
                    i + 1,
                    val,
                    err,
                    start.elapsed()
                );
                fail_count += 1;
                last_error = err.clone();

                // Check if this is the CLSE error
                if last_error.contains("CLSE") {
                    println!("     ⚠️ CLSE ERROR DETECTED - Protocol may be out of sync!");

                    // Try to recover by waiting
                    println!("     🔄 Attempting recovery (waiting)...");
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }

        // Small delay between commands to let things settle
        std::thread::sleep(Duration::from_millis(50));
    }

    println!(
        "  Summary: {} succeeded, {} failed",
        success_count, fail_count
    );
}

fn test_mixed_commands(device: &mut ADBUSBDevice) {
    // Simulate what the main app does: tap followed by screenshot

    for i in 1..=5 {
        println!("  Iteration {}:", i);

        // Simulate tap (input tap)
        let tap_start = Instant::now();
        match run_shell_cmd(device, &["input", "tap", "100", "100"]) {
            Ok(_) => println!("    ✅ Tap succeeded ({:?})", tap_start.elapsed()),
            Err(e) => {
                println!("    ❌ Tap failed: {} ({:?})", e, tap_start.elapsed());
                if e.contains("CLSE") {
                    println!("       ⚠️ CLSE ERROR - Protocol out of sync!");
                }
            }
        }

        std::thread::sleep(Duration::from_millis(100));

        // Try framebuffer capture
        let fb_start = Instant::now();
        match device.framebuffer_bytes() {
            Ok(data) => println!(
                "    ✅ Framebuffer: {} bytes ({:?})",
                data.len(),
                fb_start.elapsed()
            ),
            Err(e) => {
                let err = format!("{}", e);
                println!(
                    "    ❌ Framebuffer failed: {} ({:?})",
                    err,
                    fb_start.elapsed()
                );
                if err.contains("CLSE") {
                    println!("       ⚠️ CLSE ERROR - Protocol out of sync!");
                }
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn test_rapid_taps(device: &mut ADBUSBDevice) {
    // Test many rapid tap commands - this is what happens in automation

    let mut success_count = 0;
    let mut clse_errors = 0;
    let mut other_errors = 0;

    for i in 1..=20 {
        let x = (100 + (i * 10)).to_string();
        let y = "200";

        match run_shell_cmd(device, &["input", "tap", &x, y]) {
            Ok(_) => {
                success_count += 1;
                if i % 5 == 0 {
                    println!("  ✅ Taps 1-{}: {} succeeded", i, success_count);
                }
            }
            Err(err) => {
                if err.contains("CLSE") {
                    clse_errors += 1;
                    println!("  ❌ Tap {} CLSE ERROR at ({},{}): {}", i, x, y, err);
                } else {
                    other_errors += 1;
                    println!("  ❌ Tap {} OTHER ERROR at ({},{}): {}", i, x, y, err);
                }
            }
        }

        // Minimal delay between taps
        std::thread::sleep(Duration::from_millis(20));
    }

    println!(
        "  Summary: {} succeeded, {} CLSE errors, {} other errors",
        success_count, clse_errors, other_errors
    );

    if clse_errors > 0 {
        println!("\n  ⚠️ DIAGNOSIS: CLSE errors indicate the protocol is getting out of sync.");
        println!("     This typically happens when:");
        println!("     1. A previous command's CLSE response wasn't fully consumed");
        println!("     2. The device sends multiple CLSE messages (some devices do this)");
        println!("     3. Commands are being sent faster than responses are processed");
    }
}

fn test_long_then_short(device: &mut ADBUSBDevice) {
    // A longer command (like screencap) followed by short commands

    println!("  Running longer command (screencap partial)...");
    let start = Instant::now();

    // Use timeout wrapper for screencap which can hang on some devices
    // The shell command takes separate arguments
    let screencap_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_shell_cmd(
            device,
            &["sh", "-c", "screencap -p 2>/dev/null | head -c 1000"],
        )
    }));

    match screencap_result {
        Ok(Ok(output)) => println!(
            "  ✅ Screencap partial completed ({:?}, {} bytes)",
            start.elapsed(),
            output.len()
        ),
        Ok(Err(e)) => println!("  ⚠️ Screencap failed (may be expected): {}", e),
        Err(_) => println!("  ⚠️ Screencap panicked"),
    }

    std::thread::sleep(Duration::from_millis(200));

    // Now try short commands
    println!("  Following up with short commands...");
    for i in 1..=3 {
        match run_shell_cmd(device, &["echo", "quick_test"]) {
            Ok(output) => println!("    ✅ Quick command {}: {}", i, output.trim()),
            Err(e) => {
                println!("    ❌ Quick command {} failed: {}", i, e);
                if e.contains("CLSE") {
                    println!("       ⚠️ Protocol sync issue after long command!");
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
