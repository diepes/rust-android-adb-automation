// https://crates.io/crates/adb_client
use crate::adb::{AdbClient, Device};
// use tokio::process::Command;
use adb_client::{ADBDeviceExt, ADBServer, ADBServerDevice};
use std::sync::Arc;
use tokio::sync::Mutex;

#[allow(dead_code)]
pub struct RustAdb {
    device: Device,
    server: Arc<Mutex<ADBServer>>, // manage server instance
    server_device: Arc<Mutex<ADBServerDevice>>, // underlying connected device
    screen_x: u32,
    screen_y: u32,
}

impl RustAdb {

    async fn get_screen_size_with(&self) -> Result<(u32, u32), String> {
        // Use device shell_command instead of external adb binary
        let mut out: Vec<u8> = Vec::new();
        {
            let mut dev = self.server_device.lock().await;
            // wm size returns text
            dev.shell_command(&["wm", "size"], &mut out)
                .map_err(|e| format!("RustAdb: wm size failed: {e}"))?;
        }
        let stdout = String::from_utf8_lossy(&out);
        for line in stdout.lines() {
            if let Some(size_str) = line.strip_prefix("Physical size: ") {
                let parts: Vec<&str> = size_str.trim().split('x').collect();
                if parts.len() == 2
                    && let (Ok(x), Ok(y)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                        return Ok((x, y));
                    }
            }
        }
        Err("RustAdb: could not parse screen size".into())
    }

    async fn capture_screen_bytes_internal(&self) -> Result<Vec<u8>, String> {
        let mut out: Vec<u8> = Vec::new();
        let mut dev = self.server_device.lock().await;
        dev.shell_command(&["screencap", "-p"], &mut out)
            .map_err(|e| format!("RustAdb: screencap failed: {e}"))?;
        Ok(out)
    }
}

impl AdbClient for RustAdb {
    async fn list_devices() -> Result<Vec<Device>, String> {
        let mut server = ADBServer::default();
        let result = tokio::task::spawn_blocking(move || server.devices())
            .await
            .map_err(|e| format!("RustAdb: join error: {e}"))?;
        let device_list = result.map_err(|e| format!("RustAdb: devices failed: {e}"))?;
        let mapped = device_list
            .into_iter()
            .map(|d| Device {
                name: d.identifier,
                transport_id: None,
            })
            .collect();
        Ok(mapped)
    }

    async fn new_with_device(device_name: &str) -> Result<Self, String> {
        let mut server = ADBServer::default();
        // get_device_by_name or get_device depending on provided name
        let server_device = tokio::task::spawn_blocking({
            let name = device_name.to_string();
            move || {
                if name.is_empty() {
                    server.get_device()
                } else {
                    server.get_device_by_name(&name)
                }
                .map(|dev| (server, dev))
            }
        })
        .await
        .map_err(|e| format!("RustAdb: join error: {e}"))?
        .map_err(|e| format!("RustAdb: open device failed: {e}"))?;
        let (srv, dev) = server_device;
        let tmp = RustAdb {
            device: Device {
                name: device_name.to_string(),
                transport_id: None,
            },
            server: Arc::new(Mutex::new(srv)),
            server_device: Arc::new(Mutex::new(dev)),
            screen_x: 0,
            screen_y: 0,
        };
        let (sx, sy) = tmp.get_screen_size_with().await?;
        Ok(RustAdb {
            screen_x: sx,
            screen_y: sy,
            ..tmp
        })
    }

    async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String> {
        self.capture_screen_bytes_internal().await
    }

    async fn tap(&self, x: u32, y: u32) -> Result<(), String> {
        if x > self.screen_x || y > self.screen_y {
            return Err(format!("RustAdb: tap out of bounds x={x} y={y}"));
        }
        let mut out: Vec<u8> = Vec::new();
        let mut dev = self.server_device.lock().await;
        let xs = x.to_string();
        let ys = y.to_string();
        dev.shell_command(&["input", "tap", &xs, &ys], &mut out)
            .map_err(|e| format!("RustAdb: tap failed: {e}"))?;
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
        let mut out: Vec<u8> = Vec::new();
        let mut dev = self.server_device.lock().await;
        let s1 = x1.to_string();
        let s2 = y1.to_string();
        let s3 = x2.to_string();
        let s4 = y2.to_string();
        let mut cmd_parts: Vec<String> = vec!["input".into(), "swipe".into(), s1, s2, s3, s4];
        if let Some(d) = duration {
            cmd_parts.push(d.to_string());
        }
        let refs: Vec<&str> = cmd_parts.iter().map(|s| s.as_str()).collect();
        dev.shell_command(&refs, &mut out)
            .map_err(|e| format!("RustAdb: swipe failed: {e}"))?;
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
