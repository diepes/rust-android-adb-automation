use android_adb_run::adb::Adb;

fn main() {
    match Adb::new() {
        Ok(ref adb) => {
            if let Some(ref device) = adb.device {
                println!("Connected to device: {} (transport_id: {:?})", device.name, device.transport_id);
                match adb.screen_capture("test-android.png") {
                    Ok(_) => println!("Screen capture saved to test-android.png"),
                    Err(e) => println!("Screen capture failed: {}", e),
                }
            } else {
                println!("Connected, but no device info available.");
            }
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}
