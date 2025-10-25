use serde::Serialize;

// Trait defining ADB capabilities (shell or rust implementations)
#[allow(async_fn_in_trait)]
pub trait AdbClient: Send + Sync {
    async fn list_devices() -> Result<Vec<Device>, String> where Self: Sized;
    async fn new_with_device(device_name: &str) -> Result<Self, String> where Self: Sized;
    async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String>;
    async fn tap(&self, x: u32, y: u32) -> Result<(), String>;
    async fn swipe(&self, x1: u32, y1: u32, x2: u32, y2: u32, duration: Option<u32>) -> Result<(), String>;
    fn screen_dimensions(&self) -> (u32, u32);
    fn device_name(&self) -> &str;
}

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct Device {
    pub name: String,
    pub transport_id: Option<String>,
}

// Re-export shell implementation so existing code using `Adb` keeps working.
pub use crate::adb_shell::AdbShell as Adb;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_devices_multiple() {
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\noneplus6:5555          device product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:3\n";
        let devices = Adb::parse_devices(adb_output);
        assert_eq!(
            devices,
            vec![
                Device {
                    name: "1d36d8f1".to_string(),
                    transport_id: Some("2".to_string())
                },
                Device {
                    name: "oneplus6:5555".to_string(),
                    transport_id: Some("3".to_string())
                },
            ]
        );
    }

    #[test]
    fn test_parse_devices_single() {
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\n";
        let devices = Adb::parse_devices(adb_output);
        assert_eq!(
            devices,
            vec![Device {
                name: "1d36d8f1".to_string(),
                transport_id: Some("2".to_string())
            }]
        );
    }

    #[test]
    fn test_list_devices_mock() {
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\noneplus6:5555          device product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:3\n";
        let devices = Adb::parse_devices(adb_output);
        assert_eq!(
            devices,
            vec![
                Device {
                    name: "1d36d8f1".to_string(),
                    transport_id: Some("2".to_string())
                },
                Device {
                    name: "oneplus6:5555".to_string(),
                    transport_id: Some("3".to_string())
                },
            ]
        );
    }
}
