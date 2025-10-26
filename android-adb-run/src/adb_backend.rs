use crate::adb::{AdbClient, Device};
use crate::adb_client::RustAdb;
use crate::adb_shell::AdbShell;

pub enum AdbBackend {
    Shell(AdbShell),
    Rust(RustAdb),
}

pub use AdbBackend as Backend;

impl AdbBackend {
    pub async fn list_devices(impl_choice: &str) -> Result<Vec<Device>, String> {
        match impl_choice {
            "shell" => AdbShell::list_devices().await,
            _ => RustAdb::list_devices().await,
        }
    }

    pub async fn connect_first(impl_choice: &str) -> Result<Self, String> {
        let devices = Self::list_devices(impl_choice).await?;
        let first = devices
            .into_iter()
            .next()
            .ok_or_else(|| "No devices found".to_string())?;
        Self::new_with_device(impl_choice, &first.name).await
    }

    pub async fn new_with_device(impl_choice: &str, name: &str) -> Result<Self, String> {
        match impl_choice {
            "shell" => {
                let shell = AdbShell::new_with_device(name).await?;
                Ok(AdbBackend::Shell(shell))
            }
            _ => {
                let rust = RustAdb::new_with_device(name).await?;
                Ok(AdbBackend::Rust(rust))
            }
        }
    }

    pub fn device_name(&self) -> &str {
        match self {
            AdbBackend::Shell(s) => s.device_name(),
            AdbBackend::Rust(r) => r.device_name(),
        }
    }
    pub fn screen_dimensions(&self) -> (u32, u32) {
        match self {
            AdbBackend::Shell(s) => s.screen_dimensions(),
            AdbBackend::Rust(r) => r.screen_dimensions(),
        }
    }
    pub fn transport_id(&self) -> Option<u32> {
        match self {
            AdbBackend::Shell(s) => Some(s.transport_id),
            AdbBackend::Rust(r) => r.transport_id(),
        }
    }
    pub async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            AdbBackend::Shell(s) => s.screen_capture_bytes().await,
            AdbBackend::Rust(r) => r.screen_capture_bytes().await,
        }
    }
    pub async fn tap(&self, x: u32, y: u32) -> Result<(), String> {
        match self {
            AdbBackend::Shell(s) => s.tap(x, y).await,
            AdbBackend::Rust(r) => r.tap(x, y).await,
        }
    }
    pub async fn swipe(
        &self,
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        duration: Option<u32>,
    ) -> Result<(), String> {
        match self {
            AdbBackend::Shell(s) => s.swipe(x1, y1, x2, y2, duration).await,
            AdbBackend::Rust(r) => r.swipe(x1, y1, x2, y2, duration).await,
        }
    }
}
