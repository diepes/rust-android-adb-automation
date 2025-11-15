// Core ADB types and traits
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize)]
pub struct ImageCapture {
    pub bytes: Vec<u8>,
    pub duration_ms: u128,
    pub index: u64, // sequential capture count (per backend instance)
}

// Touch activity monitoring state
#[derive(Debug, Clone)]
pub struct TouchActivityState {
    pub last_touch_time: Option<Instant>,
    pub is_monitoring: bool,
    pub timeout_duration: Duration,
}

impl TouchActivityState {
    pub fn new(timeout_seconds: u64) -> Self {
        Self {
            last_touch_time: None,
            is_monitoring: false,
            timeout_duration: Duration::from_secs(timeout_seconds),
        }
    }

    pub fn is_human_active(&self) -> bool {
        if let Some(last_touch) = self.last_touch_time {
            last_touch.elapsed() < self.timeout_duration
        } else {
            false
        }
    }

    pub fn mark_touch_activity(&mut self) {
        self.last_touch_time = Some(Instant::now());
    }

    pub fn update_activity(&mut self) {
        self.mark_touch_activity();
    }

    pub fn has_activity_expired(&self) -> bool {
        if let Some(last_touch) = self.last_touch_time {
            last_touch.elapsed() >= self.timeout_duration
        } else {
            false
        }
    }

    pub fn get_remaining_seconds(&self) -> Option<u64> {
        if let Some(last_touch) = self.last_touch_time {
            let elapsed = last_touch.elapsed();
            if elapsed < self.timeout_duration {
                let remaining = self.timeout_duration - elapsed;
                Some(remaining.as_secs())
            } else {
                None
            }
        } else {
            None
        }
    }
}

// Shared touch activity monitor type
pub type TouchActivityMonitor = Arc<RwLock<TouchActivityState>>;

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
    async fn get_device_ip(&self) -> Result<String, String>;

    // Touch activity monitoring methods
    async fn is_human_touching(&self) -> bool;
    async fn get_touch_timeout_remaining(&self) -> Option<u64>;
    async fn start_touch_monitoring(&self) -> Result<(), String>;
    async fn stop_touch_monitoring(&self) -> Result<(), String>;

    fn screen_dimensions(&self) -> (u32, u32);
    fn device_name(&self) -> &str;
    fn transport_id(&self) -> Option<u32>; // new optional shell-specific identifier
}

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct Device {
    pub name: String,
    pub transport_id: Option<String>,
}
