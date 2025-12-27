use android_adb_run::adb::{AdbBackend, AdbClient};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing Tap Timeout with claim_1d_tap coordinates");
    println!("====================================================\n");

    // Initialize ADB
    println!("📱 Initializing ADB connection...");
    let adb = AdbBackend::connect_first().await?;
    println!("✅ ADB initialized\n");

    let (x, y) = (350, 628); // claim_1d_tap coordinates

    println!("🎯 Test 1: Normal tap (device connected)");
    println!("   Tapping at ({}, {})...", x, y);
    let start = Instant::now();
    match adb.tap(x, y).await {
        Ok(_) => println!("✅ Tap succeeded in {:?}\n", start.elapsed()),
        Err(e) => println!("❌ Tap failed: {} (after {:?})\n", e, start.elapsed()),
    }

    println!("⏳ Waiting 2 seconds...\n");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    println!("🔌 Test 2: Tap with USB disconnected");
    println!("   ⚠️  UNPLUG USB CABLE NOW!");
    println!("   Waiting 10 seconds for you to unplug...\n");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    println!("🎯 Attempting tap at ({}, {}) with USB unplugged...", x, y);
    println!("   Expected: Timeout after 5 seconds");
    let start = Instant::now();

    match adb.tap(x, y).await {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("✅ Tap succeeded in {:?}", elapsed);
            println!("   (Device was probably still connected)");
        }
        Err(e) => {
            let elapsed = start.elapsed();
            println!("❌ Tap failed after {:?}", elapsed);
            println!("   Error: {}", e);

            // Check error detection
            if e.to_string().to_lowercase().contains("timeout")
                || e.to_string().to_lowercase().contains("timed out")
            {
                println!("   ✅ Timeout detected correctly!");
            } else if e.to_string().to_lowercase().contains("offline")
                || e.to_string().to_lowercase().contains("disconnected")
                || e.to_string().to_lowercase().contains("not found")
            {
                println!("   ✅ Disconnect detected correctly!");
            } else {
                println!("   ⚠️  Error might not be detected as disconnect");
            }

            // Check timing
            if elapsed.as_secs() >= 5 {
                println!("   ✅ Timeout occurred at expected time (5+ seconds)");
            } else if elapsed.as_secs() < 1 {
                println!("   ℹ️  Fast failure (immediate error)");
            } else {
                println!("   ⚠️  Unexpected timing: {:?}", elapsed);
            }
        }
    }

    println!("\n🎯 Test 3: Retry tap (verify it's still disconnected)");
    let start = Instant::now();
    match adb.tap(x, y).await {
        Ok(_) => println!("✅ Tap succeeded - device reconnected?"),
        Err(e) => {
            println!("❌ Tap failed after {:?}", start.elapsed());
            println!("   Error: {}", e);
        }
    }

    println!("\n✅ Test complete!");
    println!("\nIf timeout didn't work:");
    println!("  1. Check if spawn_blocking is being used");
    println!("  2. Verify timeout wrapper is in place");
    println!("  3. Check if blocking_lock() is used (not lock().await)");

    Ok(())
}
