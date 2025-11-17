// Test application for ADB USB disconnect detection
// Run with: cargo run --example test_adb_disconnect

use android_adb_run::adb::AdbBackend;
use std::io::{self, Write};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔌 ADB USB Disconnect Detection Test");
    println!("=====================================\n");

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

    println!("\n🧪 Starting disconnect detection test...");
    println!("Instructions:");
    println!("  1. Keep the device connected for a few iterations");
    println!("  2. Unplug the USB cable when ready");
    println!("  3. Watch for disconnect detection\n");

    let mut iteration = 0;
    let mut consecutive_errors = 0;
    
    loop {
        iteration += 1;
        print!("Iteration {}: ", iteration);
        io::stdout().flush().unwrap();

        // Try to take a screenshot (this will fail if device is disconnected)
        match adb_client.screen_capture_bytes().await {
            Ok(bytes) => {
                println!("✅ Screenshot successful ({} bytes)", bytes.len());
                consecutive_errors = 0;
            }
            Err(e) => {
                println!("❌ Error: {}", e);
                consecutive_errors += 1;

                // Check if this is a disconnect error
                if android_adb_run::game_automation::fsm::is_disconnect_error(&e) {
                    println!("\n🔌 DISCONNECT DETECTED!");
                    println!("Error message: '{}'", e);
                    println!("\n✅ Disconnect detection is working correctly!");
                    
                    // Wait a bit to see if we can reconnect
                    println!("\n🔄 Attempting to reconnect in 5 seconds...");
                    sleep(Duration::from_secs(5)).await;
                    
                    match AdbBackend::connect_first(true).await {
                        Ok(client) => {
                            println!("✅ Reconnected successfully!");
                            adb_client = client;
                            consecutive_errors = 0;
                        }
                        Err(e) => {
                            println!("❌ Reconnection failed: {}", e);
                            println!("Please reconnect the device and restart the test.");
                            break;
                        }
                    }
                } else {
                    println!("⚠️ Non-disconnect error detected");
                }

                // Exit after too many consecutive errors
                if consecutive_errors >= 5 {
                    println!("\n❌ Too many consecutive errors. Exiting.");
                    break;
                }
            }
        }

        // Wait before next iteration
        sleep(Duration::from_secs(2)).await;
    }

    println!("\n🏁 Test completed");
    Ok(())
}
