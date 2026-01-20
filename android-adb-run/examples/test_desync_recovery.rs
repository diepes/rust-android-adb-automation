/// Test to verify ADB protocol desync detection and recovery
///
/// This test demonstrates:
/// 1. How protocol desync errors (CLSE) are detected
/// 2. How to recover from them by reconnecting
///
/// Run with: cargo run --example test_desync_recovery
use adb_client::{ADBDeviceExt, ADBUSBDevice};
use std::time::{Duration, Instant};

fn main() {
    println!("🔍 ADB Protocol Desync Detection & Recovery Test");
    println!("=================================================\n");

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

    // Test the detection and recovery flow
    let mut connection_attempts: u32 = 0;
    let max_reconnects: u32 = 3;

    loop {
        connection_attempts += 1;
        println!(
            "🔌 Connection attempt {} of {}",
            connection_attempts,
            max_reconnects + 1
        );

        let mut device = match connect_with_retry(&key_path, 3) {
            Some(d) => d,
            None => {
                if connection_attempts <= max_reconnects {
                    println!("⚠️ Connection failed, will retry...\n");
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                } else {
                    println!("❌ Max reconnection attempts reached");
                    return;
                }
            }
        };
        println!("✅ Connected!\n");

        // Run tests until we hit a desync or complete successfully
        match run_command_series(&mut device) {
            TestResult::Success => {
                println!("\n✅ All commands completed successfully!");
                break;
            }
            TestResult::ProtocolDesync => {
                println!("\n⚠️ Protocol desync detected!");
                if connection_attempts <= max_reconnects {
                    println!("🔄 Attempting to recover by reconnecting...\n");
                    // Drop device to close connection
                    drop(device);
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                } else {
                    println!("❌ Max reconnection attempts reached");
                    break;
                }
            }
            TestResult::OtherError(msg) => {
                println!("\n❌ Non-recoverable error: {}", msg);
                break;
            }
        }
    }

    println!("\n📋 Summary:");
    println!("   Total connection attempts: {}", connection_attempts);
    println!(
        "   Reconnects needed: {}",
        connection_attempts.saturating_sub(1)
    );
}

enum TestResult {
    Success,
    ProtocolDesync,
    OtherError(String),
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
                // CLSE during connection is actually expected if there's stale state
                if err_msg.contains("CLSE") {
                    println!(
                        "  ⚠️ Attempt {}/{}: Stale CLSE (will retry)",
                        attempt, max_attempts
                    );
                } else {
                    println!("  ⚠️ Attempt {}/{}: {}", attempt, max_attempts, err_msg);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
    None
}

fn run_command_series(device: &mut ADBUSBDevice) -> TestResult {
    println!("📋 Running command series...");

    // First, test basic connectivity
    println!("  1. Testing basic echo command...");
    match run_shell_cmd(device, &["echo", "hello"]) {
        Ok(output) => println!("     ✅ Echo: {}", output.trim()),
        Err(e) if is_desync_error(&e) => {
            println!("     ❌ Echo failed with protocol desync: {}", e);
            return TestResult::ProtocolDesync;
        }
        Err(e) => {
            println!("     ❌ Echo failed: {}", e);
            return TestResult::OtherError(e);
        }
    }

    // Test tap command
    println!("  2. Testing tap command...");
    match run_shell_cmd(device, &["input", "tap", "100", "100"]) {
        Ok(_) => println!("     ✅ Tap executed"),
        Err(e) if is_desync_error(&e) => {
            println!("     ❌ Tap failed with protocol desync: {}", e);
            return TestResult::ProtocolDesync;
        }
        Err(e) => {
            println!("     ❌ Tap failed: {}", e);
            return TestResult::OtherError(e);
        }
    }

    // Short delay
    std::thread::sleep(Duration::from_millis(50));

    // Test framebuffer (this often triggers desync issues)
    println!("  3. Testing framebuffer capture...");
    let fb_start = Instant::now();
    match device.framebuffer_bytes() {
        Ok(data) => {
            println!(
                "     ✅ Framebuffer: {} bytes ({:?})",
                data.len(),
                fb_start.elapsed()
            );
        }
        Err(e) => {
            let err_str = format!("{}", e);
            if is_desync_error(&err_str) {
                println!(
                    "     ❌ Framebuffer failed with protocol desync: {}",
                    err_str
                );
                return TestResult::ProtocolDesync;
            }
            // Framebuffer failure might be OK, try screencap fallback
            println!(
                "     ⚠️ Framebuffer failed ({}), trying screencap...",
                err_str
            );
            match run_shell_cmd(device, &["screencap", "-p"]) {
                Ok(data) => println!("     ✅ Screencap: {} bytes", data.len()),
                Err(e) if is_desync_error(&e) => {
                    println!("     ❌ Screencap failed with protocol desync: {}", e);
                    return TestResult::ProtocolDesync;
                }
                Err(e) => {
                    println!("     ❌ Screencap also failed: {}", e);
                    return TestResult::OtherError(e);
                }
            }
        }
    }

    // Test rapid taps (stress test)
    println!("  4. Testing rapid tap sequence...");
    for i in 1..=5 {
        let x = (100 + i * 10).to_string();
        match run_shell_cmd(device, &["input", "tap", &x, "200"]) {
            Ok(_) => {
                if i == 5 {
                    println!("     ✅ All 5 rapid taps succeeded");
                }
            }
            Err(e) if is_desync_error(&e) => {
                println!("     ❌ Tap {} failed with protocol desync: {}", i, e);
                return TestResult::ProtocolDesync;
            }
            Err(e) => {
                println!("     ❌ Tap {} failed: {}", i, e);
                return TestResult::OtherError(e);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    // Final echo to verify connection still works
    println!("  5. Final connectivity check...");
    match run_shell_cmd(device, &["echo", "done"]) {
        Ok(output) => println!("     ✅ Final echo: {}", output.trim()),
        Err(e) if is_desync_error(&e) => {
            println!("     ❌ Final echo failed with protocol desync: {}", e);
            return TestResult::ProtocolDesync;
        }
        Err(e) => {
            println!("     ❌ Final echo failed: {}", e);
            return TestResult::OtherError(e);
        }
    }

    TestResult::Success
}

fn run_shell_cmd(device: &mut ADBUSBDevice, args: &[&str]) -> Result<String, String> {
    let mut out = Vec::new();
    match device.shell_command(args, &mut out) {
        Ok(_) => Ok(String::from_utf8_lossy(&out).to_string()),
        Err(e) => Err(format!("{}", e)),
    }
}

fn is_desync_error(err: &str) -> bool {
    err.contains("CLSE") || err.contains("no write endpoint") || err.contains("wrong command")
}
