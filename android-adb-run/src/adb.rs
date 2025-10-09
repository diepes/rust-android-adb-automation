use tokio::process::Command;
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct Device {
    pub name: String,
    pub transport_id: Option<String>,
}

pub struct Adb {
    pub device: Device,
    pub transport_id: u32,
    pub screen_x: u32,
    pub screen_y: u32,
}

impl Adb {
    pub async fn new(transport_id: Option<&str>) -> Result<Self, String> {
        let devices = Self::list_devices().await?;
        if devices.is_empty() {
            return Err("No devices available".to_string());
        }
        let device = match transport_id {
            Some(tid) => devices.into_iter().find(|d| d.transport_id.as_deref() == Some(tid)),
            None => devices.into_iter().next(),
        };
        let device = match device {
            Some(d) => d,
            None => return Err("Device with specified transport_id not found".to_string()),
        };
        let transport_id = match &device.transport_id {
            Some(tid_str) => tid_str.parse::<u32>().map_err(|_| "Invalid transport_id format".to_string())?,
            None => return Err("Device missing transport_id".to_string()),
        };
        let (screen_x, screen_y) = Self::get_screen_size().await?;
        Ok(Adb {
            transport_id,
            device,
            screen_x,
            screen_y,
        })
    }

    async fn get_screen_size() -> Result<(u32, u32), String> {
        let output = Command::new("adb")
            .arg("shell")
            .arg("wm")
            .arg("size")
            .output()
            .await
            .map_err(|e| format!("Failed to run adb shell wm size: {}", e))?;
        if !output.status.success() {
            return Err(format!("adb shell wm size failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Example output: "Physical size: 1080x2400"
        for line in stdout.lines() {
            if let Some(size_str) = line.strip_prefix("Physical size: ") {
                let parts: Vec<&str> = size_str.trim().split('x').collect();
                if parts.len() == 2 {
                    if let (Ok(x), Ok(y)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                        return Ok((x, y));
                    }
                }
            }
        }
        Err("Could not parse screen size".to_string())
    }

    pub async fn new_with_device(device_name: &str) -> Result<Self, String> {
        // First, list devices
        let devices = Self::list_devices().await?;
        if let Some(device) = devices.iter().find(|d| d.name == device_name) {
            return Self::new(device.transport_id.as_deref()).await;
        }
        // Try to connect
        let output = Command::new("adb")
            .arg("connect")
            .arg(device_name)
            .output()
            .await
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
        let devices = Self::list_devices().await?;
        if let Some(device) = devices.iter().find(|d| d.name == device_name) {
            return Self::new(device.transport_id.as_deref()).await;
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

    pub async fn list_devices() -> Result<Vec<Device>, String> {
        let output = Command::new("adb")
            .arg("devices")
            .arg("-l")
            .output()
            .await
            .map_err(|e| format!("Failed to execute adb: {}", e))?;
        if !output.status.success() {
            return Err(format!("adb devices failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(Self::parse_devices(&stdout))
    }

    pub async fn screen_capture(&self, output_path: &str) -> Result<(), String> {
        let png_data = self.screen_capture_bytes().await?;
        tokio::fs::write(output_path, &png_data)
            .await
            .map_err(|e| format!("Failed to write PNG file: {}", e))?;
        Ok(())
    }

    pub async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String> {
        let mut cmd = Command::new("adb");
        cmd.arg("-t").arg(self.transport_id.to_string());
        let output = cmd
            .arg("exec-out")
            .arg("screencap")
            .arg("-p")
            .output()
            .await
            .map_err(|e| format!("Failed to run adb screencap: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "adb screencap failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(output.stdout)
    }

    pub async fn tap(&self, x: u32, y: u32) -> Result<(), String> {
        if x > self.screen_x || y > self.screen_y {
            return Err(format!(
                "Coordinates out of bounds: x={}, y={}, screen_x={}, screen_y={}",
                x, y, self.screen_x, self.screen_y
            ));
        }
        let output = Command::new("adb")
            .arg("shell")
            .arg("input")
            .arg("tap")
            .arg(x.to_string())
            .arg(y.to_string())
            .output()
            .await
            .map_err(|e| format!("Failed to run adb shell input tap: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "adb tap failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }

    pub async fn swipe(&self, x1: u32, y1: u32, x2: u32, y2: u32, duration: Option<u32>) -> Result<(), String> {
        if x1 > self.screen_x || y1 > self.screen_y || x2 > self.screen_x || y2 > self.screen_y {
            return Err(format!(
                "Swipe coordinates out of bounds: x1={}, y1={}, x2={}, y2={}, screen_x={}, screen_y={}",
                x1, y1, x2, y2, self.screen_x, self.screen_y
            ));
        }
        let mut cmd = Command::new("adb");
        cmd.arg("shell")
            .arg("input")
            .arg("swipe")
            .arg(x1.to_string())
            .arg(y1.to_string())
            .arg(x2.to_string())
            .arg(y2.to_string());
        if let Some(d) = duration {
            cmd.arg(d.to_string());
        }
        let output = cmd.output().await.map_err(|e| format!("Failed to run adb shell input swipe: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "adb swipe failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
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
