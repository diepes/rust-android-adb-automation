// Core ADB types and traits
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ImageCapture {
    pub bytes: Vec<u8>,
    pub duration_ms: u128,
    pub index: u64, // sequential capture count (per backend instance)
}

// Trait defining ADB capabilities (shell or rust implementations)
#[allow(async_fn_in_trait)]
pub trait AdbClient: Send + Sync {
    async fn list_devices() -> Result<Vec<Device>, String>
    where
        Self: Sized;
    async fn new_with_device(device_name: &str) -> Result<Self, String>
    where
        Self: Sized;

    // Raw backend-specific capture (implemented per backend)
    async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String>;

    // Default high-level capture with timing (index now managed by GUI)
    async fn screen_capture(&self) -> Result<ImageCapture, String> {
        let start = std::time::Instant::now();
        let bytes = self.screen_capture_bytes().await?;
        let dur = start.elapsed().as_millis();
        Ok(ImageCapture {
            bytes,
            duration_ms: dur,
            index: 0, // Index is now managed by GUI, this is unused
        })
    }

    async fn tap(&self, x: u32, y: u32) -> Result<(), String>;
    async fn swipe(
        &self,
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        duration: Option<u32>,
    ) -> Result<(), String>;
    fn screen_dimensions(&self) -> (u32, u32);
    fn device_name(&self) -> &str;
    fn transport_id(&self) -> Option<u32>; // new optional shell-specific identifier
}

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct Device {
    pub name: String,
    pub transport_id: Option<String>,
}
