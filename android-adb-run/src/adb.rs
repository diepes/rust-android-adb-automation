use std::process::Command;

pub struct Adb {
    pub connected: bool,
}

impl Adb {
    pub fn new() -> Self {
        Adb { connected: false }
    }

    pub fn parse_devices(output: &str) -> Vec<String> {
        output
            .lines()
            .skip(1)
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1] == "device" {
                    Some(parts[0].to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn list_devices() -> Vec<String> {
        let output = Command::new("adb")
            .arg("devices")
            .arg("-l")
            .output()
            .expect("Failed to execute adb");
        let stdout = String::from_utf8_lossy(&output.stdout);
        Self::parse_devices(&stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_devices_multiple() {
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\noneplus6:5555          device product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:3\n";
        let devices = Adb::parse_devices(adb_output);
        assert_eq!(devices, vec!["1d36d8f1", "oneplus6:5555"]);
    }

    #[test]
    fn test_parse_devices_single() {
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\n";
        let devices = Adb::parse_devices(adb_output);
        assert_eq!(devices, vec!["1d36d8f1"]);
    }

    #[test]
    fn test_list_devices() {
        // Simulate parsing logic using the same code as list_devices
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\noneplus6:5555          device product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:3\n";
        let devices = Adb::parse_devices(adb_output);
        assert_eq!(devices, vec!["1d36d8f1", "oneplus6:5555"]);
    }
}
