// ADB backend - direct USB only (no daemon required)
use super::types::AdbClient;
use super::usb_impl::UsbAdb;

/// AdbBackend is now just a type alias for UsbAdb (direct USB connection)
pub type AdbBackend = UsbAdb;

impl AdbBackend {
    /// Connect to the first available USB device
    pub async fn connect_first() -> Result<Self, String> {
        let devices = Self::list_devices().await?;
        let first = devices
            .into_iter()
            .next()
            .ok_or_else(|| "No USB devices found".to_string())?;
        Self::new_with_device(&first.name).await
    }
}

// Re-export Backend alias for backward compatibility
pub use AdbBackend as Backend;
