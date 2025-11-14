// https://crates.io/crates/adb_client
use super::types::{AdbClient, Device, TouchActivityMonitor, TouchActivityState};
use adb_client::{ADBDeviceExt, ADBServer, ADBServerDevice};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

#[allow(dead_code)]
pub struct RustAdb {
    device: Device,
    server: Arc<Mutex<ADBServer>>, // manage server instance
    server_device: Arc<Mutex<ADBServerDevice>>, // underlying connected device
    screen_x: u32,
    screen_y: u32,
    touch_monitor: TouchActivityMonitor,
    monitoring_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl RustAdb {
    async fn get_screen_size_with(&self) -> Result<(u32, u32), String> {
        // Use device shell_command instead of external adb binary with timeout
        let screen_size_future = async {
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
                        && let (Ok(x), Ok(y)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                    {
                        return Ok((x, y));
                    }
                }
            }
            Err("RustAdb: could not parse screen size".into())
        };

        // Add a 5 second timeout to prevent hanging
        tokio::time::timeout(std::time::Duration::from_secs(5), screen_size_future)
            .await
            .map_err(|_| "RustAdb: screen size detection timed out after 5 seconds".to_string())?
    }

    async fn capture_screen_bytes_internal(&self) -> Result<Vec<u8>, String> {
        let mut dev = self.server_device.lock().await;

        // Try the faster framebuffer_bytes() method first
        match dev.framebuffer_bytes() {
            Ok(framebuffer_data) => {
                drop(dev); // Release the lock early
                match self.framebuffer_to_png(framebuffer_data).await {
                    Ok(png_data) => return Ok(png_data),
                    Err(e) => {
                        if crate::gui::dioxus_app::is_debug_mode() {
                            eprintln!(
                                "Framebuffer conversion failed: {}, falling back to screencap",
                                e
                            );
                        }
                        // Continue to fallback method below
                    }
                }
            }
            Err(e) => {
                if crate::gui::dioxus_app::is_debug_mode() {
                    eprintln!(
                        "Framebuffer capture failed: {}, falling back to screencap",
                        e
                    );
                }
                // Continue to fallback method below
            }
        }

        // Fallback to shell screencap method
        let mut dev = self.server_device.lock().await;
        let mut out: Vec<u8> = Vec::new();
        dev.shell_command(&["screencap", "-p"], &mut out)
            .map_err(|e| format!("RustAdb: screencap fallback failed: {e}"))?;
        Ok(out)
    }

    async fn framebuffer_to_png(&self, framebuffer_data: Vec<u8>) -> Result<Vec<u8>, String> {
        use image::{ImageBuffer, codecs::png::PngEncoder};
        use std::io::Cursor;

        let pixel_count = (self.screen_x * self.screen_y) as usize;
        let data_len = framebuffer_data.len();

        // Print debug info to understand the framebuffer format (only when --debug flag is used)
        if crate::gui::dioxus_app::is_debug_mode() {
            eprintln!("DEBUG: Framebuffer analysis:");
            eprintln!(
                "  Screen dimensions: {}x{} = {} pixels",
                self.screen_x, self.screen_y, pixel_count
            );
            eprintln!("  Data length: {} bytes", data_len);
            eprintln!(
                "  Ratio: {:.2} bytes per pixel",
                data_len as f64 / pixel_count as f64
            );
        }

        // Check if this might be compressed data or a different format
        if data_len < pixel_count {
            // Check if the data is already in PNG format
            if framebuffer_data.len() >= 8 && &framebuffer_data[0..8] == b"\x89PNG\r\n\x1a\n" {
                if crate::gui::dioxus_app::is_debug_mode() {
                    eprintln!("DEBUG: Framebuffer data is already PNG format, returning as-is");
                }
                return Ok(framebuffer_data);
            }

            // Check if the data is in JPEG format
            if framebuffer_data.len() >= 2
                && framebuffer_data[0] == 0xFF
                && framebuffer_data[1] == 0xD8
            {
                if crate::gui::dioxus_app::is_debug_mode() {
                    eprintln!("DEBUG: Framebuffer data is JPEG format, converting to PNG");
                }
                return self.jpeg_to_png(framebuffer_data).await;
            }

            return Err(format!(
                "Framebuffer data appears to be compressed or in unsupported format: {} bytes for {} pixels ({:.2} bytes/pixel)",
                data_len,
                pixel_count,
                data_len as f64 / pixel_count as f64
            ));
        }

        // Handle case where framebuffer data doesn't perfectly divide by pixel count
        // This can happen when there's header information or padding
        if data_len < pixel_count * 2 {
            return Err(format!(
                "Framebuffer data too small for raw format: {} bytes for {} pixels (minimum {} bytes for RGB565)",
                data_len,
                pixel_count,
                pixel_count * 2
            ));
        }

        // Try to determine format based on data size relative to pixel count
        let (bytes_per_pixel, actual_data) = if data_len >= pixel_count * 4 {
            // Likely RGBA format, but might have extra data - use only what we need
            let start_offset = data_len - (pixel_count * 4);
            (4, &framebuffer_data[start_offset..])
        } else if data_len >= pixel_count * 3 {
            // Likely RGB format, but might have extra data - use only what we need
            let start_offset = data_len - (pixel_count * 3);
            (3, &framebuffer_data[start_offset..])
        } else if data_len >= pixel_count * 2 {
            // Likely RGB565 format, but might have extra data - use only what we need
            let start_offset = data_len - (pixel_count * 2);
            (2, &framebuffer_data[start_offset..])
        } else {
            return Err(format!(
                "Cannot determine framebuffer format: {} bytes for {} pixels",
                data_len, pixel_count
            ));
        };

        // Determine format based on bytes per pixel
        let png_data = match bytes_per_pixel {
            4 => {
                // RGBA format (most common)
                let img = ImageBuffer::<image::Rgba<u8>, _>::from_raw(
                    self.screen_x,
                    self.screen_y,
                    actual_data.to_vec(),
                )
                .ok_or("Failed to create RGBA image from framebuffer data")?;

                let mut data = Vec::new();
                let mut cursor = Cursor::new(&mut data);
                let encoder = PngEncoder::new(&mut cursor);
                img.write_with_encoder(encoder)
                    .map_err(|e| format!("Failed to encode RGBA PNG: {e}"))?;
                data
            }
            3 => {
                // RGB format
                let img = ImageBuffer::<image::Rgb<u8>, _>::from_raw(
                    self.screen_x,
                    self.screen_y,
                    actual_data.to_vec(),
                )
                .ok_or("Failed to create RGB image from framebuffer data")?;

                let mut data = Vec::new();
                let mut cursor = Cursor::new(&mut data);
                let encoder = PngEncoder::new(&mut cursor);
                img.write_with_encoder(encoder)
                    .map_err(|e| format!("Failed to encode RGB PNG: {e}"))?;
                data
            }
            2 => {
                // RGB565 format - convert to RGB
                if actual_data.len() != pixel_count * 2 {
                    return Err(format!(
                        "Invalid RGB565 data length: expected {}, got {}",
                        pixel_count * 2,
                        actual_data.len()
                    ));
                }

                let mut rgb_data = Vec::with_capacity(pixel_count * 3);
                for chunk in actual_data.chunks_exact(2) {
                    let pixel = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let r = ((pixel >> 11) & 0x1F) as u8;
                    let g = ((pixel >> 5) & 0x3F) as u8;
                    let b = (pixel & 0x1F) as u8;

                    // Convert to 8-bit values
                    rgb_data.push((r << 3) | (r >> 2));
                    rgb_data.push((g << 2) | (g >> 4));
                    rgb_data.push((b << 3) | (b >> 2));
                }

                let img = ImageBuffer::<image::Rgb<u8>, _>::from_raw(
                    self.screen_x,
                    self.screen_y,
                    rgb_data,
                )
                .ok_or("Failed to create RGB image from RGB565 data")?;

                let mut data = Vec::new();
                let mut cursor = Cursor::new(&mut data);
                let encoder = PngEncoder::new(&mut cursor);
                img.write_with_encoder(encoder)
                    .map_err(|e| format!("Failed to encode RGB565 PNG: {e}"))?;
                data
            }
            _ => {
                return Err(format!(
                    "Unsupported framebuffer format: {} bytes per pixel (total data: {}, actual data: {}, pixels: {}, screen: {}x{})",
                    bytes_per_pixel,
                    data_len,
                    actual_data.len(),
                    pixel_count,
                    self.screen_x,
                    self.screen_y
                ));
            }
        };

        Ok(png_data)
    }

    async fn jpeg_to_png(&self, jpeg_data: Vec<u8>) -> Result<Vec<u8>, String> {
        use image::{ImageFormat, codecs::png::PngEncoder};
        use std::io::Cursor;

        // Decode JPEG
        let img = image::load_from_memory_with_format(&jpeg_data, ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to decode JPEG: {e}"))?;

        // Encode as PNG
        let mut data = Vec::new();
        let mut cursor = Cursor::new(&mut data);
        let encoder = PngEncoder::new(&mut cursor);
        img.write_with_encoder(encoder)
            .map_err(|e| format!("Failed to encode JPEG as PNG: {e}"))?;

        Ok(data)
    }

    async fn monitor_touch_activity_loop(
        touch_monitor: TouchActivityMonitor,
        server_device: Arc<Mutex<ADBServerDevice>>,
    ) -> Result<(), String> {
        // Try to find an input event device that handles touch events
        let event_device = Self::find_touch_event_device(server_device.clone()).await?;
        
        if crate::gui::dioxus_app::is_debug_mode() {
            println!("🔍 Starting touch monitoring on device: {}", event_device);
        }

        loop {
            // Check if monitoring should continue
            {
                let monitor = touch_monitor.read().await;
                if !monitor.is_monitoring {
                    if crate::gui::dioxus_app::is_debug_mode() {
                        println!("🛑 Touch monitoring stopped by flag");
                    }
                    break;
                }
            }

            // Run getevent for a short duration to check for touch activity
            match Self::check_for_touch_activity(server_device.clone(), &event_device).await {
                Ok(touch_detected) => {
                    if touch_detected {
                        let mut monitor = touch_monitor.write().await;
                        monitor.mark_touch_activity();
                        if crate::gui::dioxus_app::is_debug_mode() {
                            println!("👆 Human touch activity detected");
                        }
                    }
                }
                Err(e) => {
                    if crate::gui::dioxus_app::is_debug_mode() {
                        eprintln!("⚠️ Touch activity check failed: {}", e);
                    }
                    // Wait a bit before retrying
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            }

            // Small delay to prevent excessive CPU usage
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }

    async fn find_touch_event_device(
        server_device: Arc<Mutex<ADBServerDevice>>,
    ) -> Result<String, String> {
        let mut out: Vec<u8> = Vec::new();
        let mut dev = server_device.lock().await;
        
        // List input devices and find touch device
        dev.shell_command(&["sh", "-c", "ls /dev/input/event* 2>/dev/null | head -5"], &mut out)
            .map_err(|e| format!("Failed to list input devices: {e}"))?;
        
        let output = String::from_utf8_lossy(&out);
        let devices: Vec<&str> = output.trim().split('\n').filter(|s| !s.is_empty()).collect();
        
        if devices.is_empty() {
            return Err("No input event devices found".to_string());
        }

        // Try to find a touch-capable device by checking device capabilities
        for device in &devices {
            let mut cap_out: Vec<u8> = Vec::new();
            if dev.shell_command(&["sh", "-c", &format!("timeout 1s getevent -lt {} | head -1 2>/dev/null || echo 'timeout'", device)], &mut cap_out).is_ok() {
                let cap_output = String::from_utf8_lossy(&cap_out);
                // If we get any output (not timeout), this device is accessible
                if !cap_output.contains("timeout") && !cap_output.trim().is_empty() {
                    return Ok(device.to_string());
                }
            }
        }

        // Fallback to first available device
        Ok(devices[0].to_string())
    }

    async fn check_for_touch_activity(
        server_device: Arc<Mutex<ADBServerDevice>>,
        event_device: &str,
    ) -> Result<bool, String> {
        let mut out: Vec<u8> = Vec::new();
        let mut dev = server_device.lock().await;
        
        // Run getevent for a short time to check for any touch input events
        let command = format!("timeout 0.5s getevent -lt {} 2>/dev/null || true", event_device);
        dev.shell_command(&["sh", "-c", &command], &mut out)
            .map_err(|e| format!("Failed to check touch activity: {e}"))?;
        
        let output = String::from_utf8_lossy(&out);
        
        // Check if we got any event lines indicating touch activity
        // Touch events typically contain "ABS_MT" (multi-touch) or "BTN_TOUCH" or coordinate events
        let touch_detected = output.lines().any(|line| {
            line.contains("ABS_MT") || 
            line.contains("BTN_TOUCH") || 
            line.contains("ABS_X") || 
            line.contains("ABS_Y") ||
            (line.contains("0003") && (line.contains("0035") || line.contains("0036"))) // Raw touch coordinates
        });

        if touch_detected && crate::gui::dioxus_app::is_debug_mode() {
            println!("📱 Touch event detected: {} lines", output.lines().count());
        }

        Ok(touch_detected)
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
            touch_monitor: Arc::new(RwLock::new(TouchActivityState::new(30))), // 30 second timeout
            monitoring_task: Arc::new(Mutex::new(None)),
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

    async fn get_device_ip(&self) -> Result<String, String> {
        let mut out: Vec<u8> = Vec::new();
        let mut dev = self.server_device.lock().await;
        
        // Execute: ip route | awk '{print $9}' 
        // Note: We need to use sh -c to handle the pipe and awk
        dev.shell_command(&["sh", "-c", "ip route | awk '{print $9}'"], &mut out)
            .map_err(|e| format!("RustAdb: get device IP failed: {e}"))?;
        
        let output = String::from_utf8_lossy(&out);
        let ip = output.trim();
        
        if ip.is_empty() {
            return Err("RustAdb: No IP address found".to_string());
        }
        
        // Validate that it looks like an IP address
        if ip.split('.').count() == 4 && ip.chars().all(|c| c.is_ascii_digit() || c == '.') {
            Ok(ip.to_string())
        } else {
            Err(format!("RustAdb: Invalid IP format: {}", ip))
        }
    }
    
    async fn is_human_touching(&self) -> bool {
        let monitor = self.touch_monitor.read().await;
        let is_active = monitor.is_human_active();
        
        if is_active && crate::gui::dioxus_app::is_debug_mode() {
            println!("👆 is_human_touching: TRUE - Human touch detected, automation should pause");
        }
        
        is_active
    }

    async fn start_touch_monitoring(&self) -> Result<(), String> {
        let mut monitor = self.touch_monitor.write().await;
        
        if monitor.is_monitoring {
            return Ok(()); // Already monitoring
        }
        
        monitor.is_monitoring = true;
        drop(monitor); // Release write lock

        // Clone necessary data for the background task
        let touch_monitor = Arc::clone(&self.touch_monitor);
        let server_device = Arc::clone(&self.server_device);

        // Start background monitoring task
        let task = tokio::spawn(async move {
            if let Err(e) = Self::monitor_touch_activity_loop(touch_monitor.clone(), server_device).await {
                if crate::gui::dioxus_app::is_debug_mode() {
                    eprintln!("Touch monitoring ended: {}", e);
                }
            }
            
            // Mark monitoring as stopped when task ends
            let mut monitor = touch_monitor.write().await;
            monitor.is_monitoring = false;
        });

        // Store the task handle
        let mut task_handle = self.monitoring_task.lock().await;
        *task_handle = Some(task);

        Ok(())
    }

    async fn stop_touch_monitoring(&self) -> Result<(), String> {
        let mut monitor = self.touch_monitor.write().await;
        monitor.is_monitoring = false;
        drop(monitor);

        // Cancel the background task
        let mut task_handle = self.monitoring_task.lock().await;
        if let Some(task) = task_handle.take() {
            task.abort();
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
