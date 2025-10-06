use android_adb_run::adb::Adb;

fn main() {
    match Adb::new_with_device("oneplus6:5555") {
        Ok(adb) => {
            if let Some(device) = adb.device {
                println!("Connected to device: {} (transport_id: {:?})", device.name, device.transport_id);
            } else {
                println!("Connected, but no device info available.");
            }
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}
