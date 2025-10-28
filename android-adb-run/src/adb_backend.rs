use crate::adb::{AdbClient, Device, ImageCapture};
use crate::adb_rust::RustAdb;
use crate::adb_shell::AdbShell;

pub enum AdbBackend {
    Shell(AdbShell),
    Rust(RustAdb),
}

impl AdbBackend {
    pub async fn list_devices(use_rust: bool) -> Result<Vec<Device>, String> {
        if use_rust {
            RustAdb::list_devices().await
        } else {
            AdbShell::list_devices().await
        }
    }

    pub async fn connect_first(use_rust: bool) -> Result<Self, String> {
        let devices = Self::list_devices(use_rust).await?;
        let first = devices
            .into_iter()
            .next()
            .ok_or_else(|| "No devices found".to_string())?;
        Self::new_with_device(&first.name, use_rust).await
    }

    pub async fn new_with_device(name: &str, use_rust: bool) -> Result<Self, String> {
        if use_rust {
            Ok(AdbBackend::Rust(RustAdb::new_with_device(name).await?))
        } else {
            Ok(AdbBackend::Shell(AdbShell::new_with_device(name).await?))
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
            AdbBackend::Shell(s) => s.transport_id(),
            AdbBackend::Rust(r) => r.transport_id(),
        }
    }

    pub async fn screen_capture(&self) -> Result<ImageCapture, String> {
        match self {
            AdbBackend::Shell(s) => <AdbShell as AdbClient>::screen_capture(s).await,
            AdbBackend::Rust(r) => <RustAdb as AdbClient>::screen_capture(r).await,
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

impl AdbClient for AdbBackend {
    async fn list_devices() -> Result<Vec<Device>, String>
    where
        Self: Sized,
    {
        // Default to rust implementation for backward compatibility
        AdbBackend::list_devices(true).await
    }

    async fn new_with_device(device_name: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        // Default to rust implementation for backward compatibility
        AdbBackend::new_with_device(device_name, true).await
    }

    async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String> {
        self.screen_capture_bytes().await
    }

    async fn tap(&self, x: u32, y: u32) -> Result<(), String> {
        self.tap(x, y).await
    }

    async fn swipe(
        &self,
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        duration: Option<u32>,
    ) -> Result<(), String> {
        self.swipe(x1, y1, x2, y2, duration).await
    }

    fn screen_dimensions(&self) -> (u32, u32) {
        self.screen_dimensions()
    }

    fn device_name(&self) -> &str {
        self.device_name()
    }

    fn transport_id(&self) -> Option<u32> {
        self.transport_id()
    }
}

pub use AdbBackend as Backend;
