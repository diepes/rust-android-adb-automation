// Comprehensive ADB disconnect detection test
// Tests multiple ADB operations: screenshots and taps
// Run with: cargo run --example test_adb_disconnect_comprehensive

use android_adb_run::adb::AdbBackend;
use android_adb_run::game_automation::fsm::is_disconnect_error;
use std::io::{self, Write};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔌 Comprehensive ADB USB Disconnect Detection Test");
    println!("===================================================\n");

    // Connect to ADB device
    println!("📱 Connecting to ADB device...");
    let mut adb_client = match AdbBackend::connect_first(true).await {
        Ok(client) => {
            println!("✅ Connected to ADB device");
            let (width, height) = client.screen_dimensions();
            println!("📐 Screen dimensions: {}x{}", width, height);
            client
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to ADB device: {}", e);
            eprintln!("\nMake sure:");
            eprintln!("  1. ADB device is connected via USB");
            eprintln!("  2. USB debugging is enabled");
            eprintln!("  3. Device is authorized");
            return Err(e.into());
        }
    };

    println!("\n🧪 Starting comprehensive disconnect detection test...");
    println!("This test will perform various ADB operations:");
    println!("  - Screenshots");
    println!("  - Screen taps");
    println!("  - Swipe gestures");
    println!("\nInstructions:");
    println!("  1. Keep the device connected for a few iterations");
    println!("  2. Unplug the USB cable when ready");
    println!("  3. Watch which operation detects the disconnect first\n");
    
    println!("Press Enter to start the test...");
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let mut iteration = 0;
    let mut disconnect_detected = false;
    
    loop {
        iteration += 1;
        println!("\n==================================================");
        println!("Iteration {}", iteration);
        println!("==================================================");

        // Test 1: Screenshot
        print!("  📸 Taking screenshot... ");
        io::stdout().flush().unwrap();
        match adb_client.screen_capture_bytes().await {
            Ok(bytes) => {
                println!("✅ ({} bytes)", bytes.len());
            }
            Err(e) => {
                println!("❌ FAILED");
                println!("     Error: {}", e);
                if is_disconnect_error(&e) {
                    println!("     🔌 DISCONNECT DETECTED in screenshot operation!");
                    disconnect_detected = true;
                }
            }
        }

        if disconnect_detected {
            break;
        }

        // Wait a bit between operations
        sleep(Duration::from_millis(500)).await;

        // Test 2: Tap operation
        print!("  👆 Testing tap operation... ");
        io::stdout().flush().unwrap();
        // Tap in the middle of the screen
        let (width, height) = adb_client.screen_dimensions();
        let tap_x = width / 2;
        let tap_y = height / 2;
        match adb_client.tap(tap_x, tap_y).await {
            Ok(_) => {
                println!("✅ (tapped at {}, {})", tap_x, tap_y);
            }
            Err(e) => {
                println!("❌ FAILED");
                println!("     Error: {}", e);
                if is_disconnect_error(&e) {
                    println!("     🔌 DISCONNECT DETECTED in tap operation!");
                    disconnect_detected = true;
                }
            }
        }

        if disconnect_detected {
            break;
        }

        // Wait a bit between operations
        sleep(Duration::from_millis(500)).await;

        // Test 3: Swipe operation
        print!("  ✋ Testing swipe operation... ");
        io::stdout().flush().unwrap();
        let (width, height) = adb_client.screen_dimensions();
        let swipe_x1 = width / 2;
        let swipe_y1 = height / 2;
        let swipe_x2 = width / 2;
        let swipe_y2 = height / 4;
        match adb_client.swipe(swipe_x1, swipe_y1, swipe_x2, swipe_y2, Some(300)).await {
            Ok(_) => {
                println!("✅ (swiped from {},{} to {},{})", swipe_x1, swipe_y1, swipe_x2, swipe_y2);
            }
            Err(e) => {
                println!("❌ FAILED");
                println!("     Error: {}", e);
                if is_disconnect_error(&e) {
                    println!("     🔌 DISCONNECT DETECTED in swipe operation!");
                    disconnect_detected = true;
                }
            }
        }

        if disconnect_detected {
            break;
        }

        // Wait before next iteration
        println!("\n  ⏳ Waiting 2 seconds before next iteration...");
        sleep(Duration::from_secs(2)).await;
    }

    if disconnect_detected {
        println!("\n==================================================");
        println!("✅ DISCONNECT DETECTION TEST PASSED!");
        println!("==================================================");
        println!("\nThe disconnect was successfully detected.");
        println!("The is_disconnect_error() function is working correctly.\n");
        
        // Try to reconnect
        println!("🔄 Attempting to reconnect...");
        println!("Please reconnect the USB cable and press Enter...");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        match AdbBackend::connect_first(true).await {
            Ok(client) => {
                println!("✅ Reconnected successfully!");
                let (width, height) = client.screen_dimensions();
                println!("📐 Screen dimensions: {}x{}", width, height);
                adb_client = client;
                
                // Test a screenshot to verify connection
                match adb_client.screen_capture_bytes().await {
                    Ok(bytes) => {
                        println!("✅ Post-reconnect screenshot successful ({} bytes)", bytes.len());
                        println!("\n🎉 Full reconnection test passed!");
                    }
                    Err(e) => {
                        println!("❌ Post-reconnect screenshot failed: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ Reconnection failed: {}", e);
            }
        }
    } else {
        println!("\n⚠️ Test ended without detecting disconnect");
    }

    println!("\n🏁 Test completed");
    Ok(())
}
