use android_adb_run::adb::Adb;

fn main() {
    match Adb::new(None) {
        Ok(adb) => {
            println!(
                "Connected to device: {} (transport_id: {:?}) screen size: {}x{}",
                adb.device.name,
                adb.transport_id,
                adb.screen_x,
                adb.screen_y
            );
            match adb.screen_capture("test-android.png") {
                Ok(_) => println!("Screen capture saved to test-android.png"),
                Err(e) => println!("Screen capture failed: {}", e),
            }
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}
