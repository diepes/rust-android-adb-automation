use std::process::Command;
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize)]
pub struct Device {
    pub name: String,
    pub transport_id: Option<String>,
}

pub struct Adb {
    pub connected: bool,
}

impl Adb {
    pub fn new() -> Self {
        Adb { connected: false }
    }

    pub fn parse_devices(output: &str) -> Vec<Device> {
        output
            .lines()
            .skip(1)
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1] == "device" {
                    let name = parts[0].to_string();
                    let transport_id = line.split_whitespace()
                        .find_map(|part| {
                            if part.starts_with("transport_id:") {
                                Some(part.trim_start_matches("transport_id:").to_string())
                            } else {
                                None
                            }
                        });
                    Some(Device { name, transport_id })
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn list_devices() -> Vec<Device> {
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
        assert_eq!(devices, vec![
            Device { name: "1d36d8f1".to_string(), transport_id: Some("2".to_string()) },
            Device { name: "oneplus6:5555".to_string(), transport_id: Some("3".to_string()) },
        ]);
    }

    #[test]
    fn test_parse_devices_single() {
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\n";
        let devices = Adb::parse_devices(adb_output);
        assert_eq!(devices, vec![Device { name: "1d36d8f1".to_string(), transport_id: Some("2".to_string()) }]);
    }

    #[test]
    fn test_list_devices_mock() {
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\noneplus6:5555          device product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:3\n";
        let devices = Adb::parse_devices(adb_output);
        assert_eq!(devices, vec![
            Device { name: "1d36d8f1".to_string(), transport_id: Some("2".to_string()) },
            Device { name: "oneplus6:5555".to_string(), transport_id: Some("3".to_string()) },
        ]);
    }
}
