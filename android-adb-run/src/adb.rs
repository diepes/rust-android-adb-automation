use std::process::Command;
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct Device {
    pub name: String,
    pub transport_id: Option<String>,
}

pub struct Adb {
    pub connected: bool,
    pub device: Option<Device>,
}

impl Adb {
    pub fn new() -> Result<Self, String> {
        let devices = Self::list_devices();
        if devices.is_empty() {
            return Err("No devices available".to_string());
        }
        // Use the first device found
        let device = devices.into_iter().next();
        Ok(Adb {
            connected: true,
            device,
        })
    }

    pub fn new_with_device(device_name: &str) -> Result<Self, String> {
        // First, list devices
        let mut devices = Self::list_devices();
        if let Some(device) = devices.iter().find(|d| d.name == device_name) {
            return Ok(Adb {
                connected: true,
                device: Some(device.clone()),
            });
        }
        // Try to connect
        let output = std::process::Command::new("adb")
            .arg("connect")
            .arg(device_name)
            .output()
            .map_err(|e| format!("Failed to run adb connect: {}", e))?;
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        if !output.status.success()
            || stdout_str.contains("Connection refused")
            || stderr_str.contains("Connection refused")
        {
            return Err(format!(
                "adb connect failed: Out:{}\nErr:{}\n Try: 'adb tcpip 5555'",
                stdout_str,
                stderr_str
            ));
        }
        print!("connect: {}", stdout_str);
        // List devices again
        devices = Self::list_devices();
        if let Some(device) = devices.iter().find(|d| d.name == device_name) {
            return Ok(Adb {
                connected: true,
                device: Some(device.clone()),
            });
        }
        Err(format!("Device '{}' not found after connect", device_name))
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

    pub fn screen_capture(&self, output_path: &str) -> Result<(), String> {
        let mut cmd = Command::new("adb");
        if let Some(device) = &self.device {
            if let Some(ref transport_id) = device.transport_id {
                cmd.arg("-t").arg(transport_id);
            }
        }
        let output = cmd
            .arg("exec-out")
            .arg("screencap")
            .arg("-p")
            .output()
            .map_err(|e| format!("Failed to run adb screencap: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "adb screencap failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        std::fs::write(output_path, &output.stdout)
            .map_err(|e| format!("Failed to write PNG file: {}", e))?;
        Ok(())
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
