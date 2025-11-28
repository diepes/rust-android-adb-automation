use android_adb_run::adb::{AdbClient, backend::AdbBackend};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing ADB Tap Timeout Behavior");
    println!("====================================\n");

    // Initialize ADB
    println!("📱 Initializing ADB connection...");
    let adb = AdbBackend::connect_first().await?;
    println!("✅ ADB initialized\n");

    println!("🎯 Test 1: Normal tap (device connected)");
    let start = Instant::now();
    match adb.tap(500, 500).await {
        Ok(_) => println!("✅ Tap succeeded in {:?}", start.elapsed()),
        Err(e) => println!("❌ Tap failed: {} (after {:?})", e, start.elapsed()),
    }

    println!("\n⏳ Waiting 2 seconds...\n");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    println!("🔌 Test 2: Tap during disconnect");
    println!("   INSTRUCTIONS: Unplug USB cable NOW!");
    println!("   Waiting 3 seconds for you to unplug...\n");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("🎯 Attempting tap (USB should be disconnected)...");
    let start = Instant::now();
    match adb.tap(500, 500).await {
        Ok(_) => {
            println!("✅ Tap succeeded in {:?}", start.elapsed());
            println!("   (Device was probably still connected)");
        }
        Err(e) => {
            let elapsed = start.elapsed();
            println!("❌ Tap failed: {}", e);
            println!("   Time elapsed: {:?}", elapsed);

            if elapsed.as_secs() >= 5 {
                println!("   ✅ Timeout triggered correctly (5+ seconds)");
            } else {
                println!("   ℹ️  Quick failure (immediate error detection)");
            }

            // Check if error is detected as disconnect
            if e.to_lowercase().contains("timeout")
                || e.to_lowercase().contains("offline")
                || e.to_lowercase().contains("disconnected")
            {
                println!("   ✅ Error detected as disconnect");
            } else {
                println!("   ⚠️  Error NOT detected as disconnect!");
            }
        }
    }

    println!("\n✅ Test complete!");
    println!("\nExpected behavior:");
    println!("- Test 1: Should succeed immediately");
    println!("- Test 2: Should fail after ~5 seconds with timeout error");
    println!("         OR fail immediately with device offline error");

    Ok(())
}
