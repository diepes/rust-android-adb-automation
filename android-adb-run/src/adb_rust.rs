// https://crates.io/crates/adb_client
use crate::adb::{AdbClient, Device};
use tokio::process::Command;

#[allow(dead_code)]
#[derive(Clone, PartialEq)]
pub struct RustAdb {
    device: Device,
    screen_x: u32,
    screen_y: u32,
}

impl RustAdb {
    async fn new(device: Device, screen_x: u32, screen_y: u32) -> Self {
        Self {
            device,
            screen_x,
            screen_y,
        }
    }

    async fn get_screen_size() -> Result<(u32, u32), String> {
        let output = Command::new("adb")
            .arg("shell")
            .arg("wm")
            .arg("size")
            .output()
            .await
            .map_err(|e| format!("RustAdb: wm size failed: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "RustAdb: wm size non-zero: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Reuse parsing logic similar to Adb
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
        Err("RustAdb: could not parse screen size".into())
    }
}

impl AdbClient for RustAdb {
    async fn list_devices() -> Result<Vec<Device>, String> {
        // Temporary: revert to shell invocation until adb_client API confirmed
        let output = Command::new("adb")
            .arg("devices")
            .arg("-l")
            .output()
            .await
            .map_err(|e| format!("RustAdb: adb devices failed: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "RustAdb: adb devices non-zero: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let devices = crate::adb::Adb::parse_devices(&stdout);
        Ok(devices)
    }
    async fn new_with_device(device_name: &str) -> Result<Self, String> {
        let devices = Self::list_devices().await?;
        let device = devices
            .into_iter()
            .find(|d| d.name == device_name)
            .ok_or_else(|| format!("RustAdb: device '{device_name}' not found"))?;
        let (sx, sy) = Self::get_screen_size().await?;
        Ok(RustAdb::new(device, sx, sy).await)
    }
    async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String> {
        // Simple exec-out screencap similar to Adb
        let output = Command::new("adb")
            .arg("exec-out")
            .arg("screencap")
            .arg("-p")
            .output()
            .await
            .map_err(|e| format!("RustAdb: screencap failed: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "RustAdb: screencap non-zero: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(output.stdout)
    }
    async fn tap(&self, x: u32, y: u32) -> Result<(), String> {
        if x > self.screen_x || y > self.screen_y {
            return Err(format!("RustAdb: tap out of bounds x={x} y={y}"));
        }
        let output = Command::new("adb")
            .arg("shell")
            .arg("input")
            .arg("tap")
            .arg(x.to_string())
            .arg(y.to_string())
            .output()
            .await
            .map_err(|e| format!("RustAdb: tap failed: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "RustAdb: tap non-zero: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }
    async fn swipe(
        &self,
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        duration: Option<u32>,
    ) -> Result<(), String> {
        for &(x, y) in &[(x1, y1), (x2, y2)] {
            if x > self.screen_x || y > self.screen_y {
                return Err("RustAdb: swipe out of bounds".into());
            }
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
        let output = cmd
            .output()
            .await
            .map_err(|e| format!("RustAdb: swipe failed: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "RustAdb: swipe non-zero: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }
    fn screen_dimensions(&self) -> (u32, u32) {
        (self.screen_x, self.screen_y)
    }
    fn device_name(&self) -> &str {
        &self.device.name
    }
    fn transport_id(&self) -> Option<u32> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rust_adb_list_devices_runs() {
        let result = RustAdb::list_devices().await;
        assert!(
            result.is_ok(),
            "Expected Ok listing devices, got {:?}",
            result
        );
        println!("RustAdb devices count: {}", result.unwrap().len());
    }
}
