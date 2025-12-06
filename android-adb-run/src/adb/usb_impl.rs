// Direct USB implementation without ADB daemon
use super::types::{AdbClient, Device, TouchActivityMonitor, TouchActivityState};
use adb_client::{ADBDeviceExt, ADBUSBDevice};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

pub struct UsbAdb {
    device: Device,
    usb_device: Arc<Mutex<ADBUSBDevice>>,
    screen_x: u32,
    screen_y: u32,
    touch_monitor: TouchActivityMonitor,
}

impl UsbAdb {
    async fn get_screen_size_with(&self) -> Result<(u32, u32), String> {
        let screen_size_future = async {
            let mut out: Vec<u8> = Vec::new();
            {
                let mut dev = self.usb_device.lock().await;
                dev.shell_command(&["wm", "size"], &mut out)
                    .map_err(|e| format!("UsbAdb: wm size failed: {e}"))?;
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
            Err("UsbAdb: could not parse screen size".into())
        };

        tokio::time::timeout(std::time::Duration::from_secs(5), screen_size_future)
            .await
            .map_err(|_| "UsbAdb: screen size detection timed out after 5 seconds".to_string())?
    }

    async fn capture_screen_bytes_internal(&self) -> Result<Vec<u8>, String> {
        // Use framebuffer_bytes() - it's fast and reliable
        // Note: screencap hangs forever on some devices, so we don't use it as fallback
        let mut dev = self.usb_device.lock().await;
        
        let framebuffer_data = dev.framebuffer_bytes()
            .map_err(|e| format!("UsbAdb: framebuffer capture failed: {}. Device may not support framebuffer access.", e))?;
        
        drop(dev);
        
        if crate::gui::dioxus_app::is_debug_mode() {
            eprintln!("📸 Captured {} bytes from framebuffer", framebuffer_data.len());
        }
        
        // For now, return raw framebuffer data
        // The PNG conversion can fail on compressed/unusual formats, but the raw data is valid
        match self.framebuffer_to_png(framebuffer_data.clone()).await {
            Ok(png_data) => Ok(png_data),
            Err(e) => {
                if crate::gui::dioxus_app::is_debug_mode() {
                    eprintln!("⚠️  Framebuffer to PNG conversion failed: {}", e);
                    eprintln!("💡 Raw framebuffer data is available ({} bytes), but format is unusual", framebuffer_data.len());
                }
                // Return error since we can't convert to PNG
                // In the future, could try alternative decoding methods here
                Err(format!("Framebuffer format not supported: {}", e))
            }
        }
    }    async fn framebuffer_to_png(&self, framebuffer_data: Vec<u8>) -> Result<Vec<u8>, String> {
        use image::{ImageBuffer, codecs::png::PngEncoder};
        use std::io::Cursor;

        let pixel_count = (self.screen_x * self.screen_y) as usize;
        let data_len = framebuffer_data.len();

        if crate::gui::dioxus_app::is_debug_mode() {
            eprintln!("DEBUG: Framebuffer analysis:");
            eprintln!("  Total data length: {} bytes", data_len);
            eprintln!(
                "  Screen resolution: {}x{} = {} pixels",
                self.screen_x, self.screen_y, pixel_count
            );
            eprintln!(
                "  Bytes per pixel (raw): {:.2}",
                data_len as f64 / pixel_count as f64
            );
        }

        // Try different header sizes - some devices have variable headers
        let (header_size, actual_data_len, bytes_per_pixel) = {
            let mut best_match = (0, data_len, 0);

            for header in [0, 12, 16, 20, 24] {
                if header >= data_len {
                    break;
                }
                let test_data_len = data_len - header;

                // Check if this header size gives us a valid format
                let bpp = if test_data_len >= pixel_count * 4 && test_data_len < pixel_count * 5 {
                    4 // RGBA
                } else if test_data_len >= pixel_count * 3 && test_data_len < pixel_count * 4 {
                    3 // RGB
                } else if test_data_len >= pixel_count * 2 && test_data_len < pixel_count * 3 {
                    2 // RGB565
                } else if test_data_len >= pixel_count && test_data_len < pixel_count * 2 {
                    1 // 8-bit grayscale or indexed
                } else {
                    0 // Unsupported
                };

                if bpp > 0 {
                    best_match = (header, test_data_len, bpp);
                    break;
                }
            }

            best_match
        };

        if crate::gui::dioxus_app::is_debug_mode() {
            eprintln!("  Detected header size: {} bytes", header_size);
            eprintln!("  Detected format: {} bytes per pixel", bytes_per_pixel);
            eprintln!("  Data after header: {} bytes", actual_data_len);
        }

        let actual_data = &framebuffer_data[header_size..];

        let png_data = match bytes_per_pixel {
            4 => {
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
            1 => {
                // 8-bit format - treat as grayscale
                let img = ImageBuffer::<image::Luma<u8>, _>::from_raw(
                    self.screen_x,
                    self.screen_y,
                    actual_data.to_vec(),
                )
                .ok_or("Failed to create grayscale image from 8-bit framebuffer data")?;

                let mut data = Vec::new();
                let mut cursor = Cursor::new(&mut data);
                let encoder = PngEncoder::new(&mut cursor);
                img.write_with_encoder(encoder)
                    .map_err(|e| format!("Failed to encode grayscale PNG: {e}"))?;
                data
            }
            _ => {
                return Err(format!(
                    "Unsupported framebuffer format: {} bytes per pixel (data_len={}, pixel_count={})",
                    bytes_per_pixel, actual_data_len, pixel_count
                ));
            }
        };

        Ok(png_data)
    }
}

impl AdbClient for UsbAdb {
    async fn list_devices() -> Result<Vec<Device>, String> {
        // Use blocking task for USB device enumeration with timeout
        let list_future = tokio::task::spawn_blocking(|| match adb_client::search_adb_devices() {
            Ok(Some((vendor_id, product_id))) => Ok(vec![Device {
                name: format!("{:04x}:{:04x}", vendor_id, product_id),
                transport_id: None,
            }]),
            Ok(None) => Ok(vec![]),
            Err(e) => Err(format!("UsbAdb: device enumeration failed: {e}")),
        });

        match tokio::time::timeout(Duration::from_secs(2), list_future).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(format!("UsbAdb: join error: {e}")),
            Err(_) => Err("UsbAdb: device enumeration timed out after 2 seconds".to_string()),
        }
    }

    async fn new_with_device(device_name: &str) -> Result<Self, String> {
        // Get persistent ADB key path (same as ADB daemon uses)
        let key_path = homedir::my_home()
            .ok()
            .flatten()
            .map(|home| home.join(".android").join("adbkey"))
            .ok_or("Failed to determine home directory for ADB key")?;

        if !key_path.exists() {
            return Err(format!(
                "ADB key not found at {}. Please run 'adb devices' once to generate it.",
                key_path.display()
            ));
        }

        // Try USB connection with retry logic for AUTH/CNXN/CLSE handshake issues
        eprintln!("📱 Connecting to USB device...");
        eprintln!("   Using persistent ADB key: {}", key_path.display());
        if crate::gui::dioxus_app::is_debug_mode() {
            eprintln!("   ⏱️  If you see 'Allow USB debugging?' popup, you have 10 seconds to accept");
            eprintln!("   📱 Check 'Always allow from this computer' to avoid future popups");
        }

        let mut usb_device = None;
        let max_attempts = 5;

        for attempt in 1..=max_attempts {
            let key_path_clone = key_path.clone();
            let usb_future = tokio::task::spawn_blocking(move || {
                ADBUSBDevice::autodetect_with_custom_private_key(key_path_clone)
            });

            match tokio::time::timeout(Duration::from_secs(30), usb_future).await {
                Ok(Ok(device_result)) => {
                    match device_result {
                        Ok(device) => {
                            usb_device = Some(device);
                            if attempt > 1 && crate::gui::dioxus_app::is_debug_mode() {
                                eprintln!("   ✅ Connected on attempt {}", attempt);
                            }
                            break;
                        }
                        Err(e) => {
                            let err_msg = format!("{}", e);
                            // Retry on common handshake errors (AUTH/CLSE/CNXN) and busy USB
                            let should_retry = attempt < max_attempts && (
                                err_msg.contains("Resource busy") ||
                                err_msg.contains("AUTH") ||
                                err_msg.contains("CLSE") ||
                                err_msg.contains("CNXN")
                            );
                            
                            if should_retry {
                                if crate::gui::dioxus_app::is_debug_mode() {
                                    if attempt == 1 {
                                        eprintln!("   ⚠️  Connection attempt {} failed: {}", attempt, err_msg);
                                        eprintln!("       (This is normal for first connection, retrying...)");
                                    } else {
                                        eprintln!("   ⚠️  Attempt {}/{}: {}", attempt, max_attempts, err_msg);
                                    }
                                }
                                tokio::time::sleep(Duration::from_secs(2)).await;
                            } else {
                                return Err(format!("UsbAdb: failed to connect after {} attempts: {}. Make sure USB debugging is authorized on your phone.", attempt, e));
                            }
                        }
                    }
                }
                Ok(Err(e)) => return Err(format!("UsbAdb: join error: {e}")),
                Err(_) => return Err("UsbAdb: USB device connection timed out after 30 seconds. Make sure to authorize USB debugging on your phone when prompted.".to_string()),
            }
        }

        let mut usb_device =
            usb_device.ok_or("UsbAdb: failed to establish USB connection after retries")?;

        // Test the connection with a simple command to ensure it's fully authorized
        eprintln!("🔐 Validating connection authorization...");
        let mut test_output = Vec::new();
        usb_device.shell_command(&["echo", "test"], &mut test_output)
            .map_err(|e| format!("UsbAdb: connection validation failed: {}. The device may not be properly authorized.", e))?;

        if crate::gui::dioxus_app::is_debug_mode() {
            eprintln!("✅ Connection validated - device is authorized and responsive");
        }

        let tmp = UsbAdb {
            device: Device {
                name: device_name.to_string(),
                transport_id: None,
            },
            usb_device: Arc::new(Mutex::new(usb_device)),
            screen_x: 0,
            screen_y: 0,
            touch_monitor: Arc::new(RwLock::new(TouchActivityState::new(30))),
        };

        let (sx, sy) = tmp.get_screen_size_with().await?;

        // Only log success in debug mode - backend will log the overall connection type
        if crate::gui::dioxus_app::is_debug_mode() {
            eprintln!("🔌 USB device connected: {}x{}", sx, sy);
        }

        Ok(UsbAdb {
            screen_x: sx,
            screen_y: sy,
            ..tmp
        })
    }

    async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String> {
        let capture_future = self.capture_screen_bytes_internal();

        // Increased timeout to 30s to accommodate slow screencap fallback
        match tokio::time::timeout(Duration::from_secs(30), capture_future).await {
            Ok(result) => result,
            Err(_) => Err("UsbAdb: screenshot capture timed out after 30 seconds".to_string()),
        }
    }

    async fn tap(&self, x: u32, y: u32) -> Result<(), String> {
        if x > self.screen_x || y > self.screen_y {
            return Err(format!("UsbAdb: tap out of bounds x={x} y={y}"));
        }

        let usb_device = Arc::clone(&self.usb_device);

        let tap_future = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let mut out: Vec<u8> = Vec::new();
            let mut dev = usb_device.blocking_lock();
            let xs = x.to_string();
            let ys = y.to_string();
            dev.shell_command(&["input", "tap", &xs, &ys], &mut out)
                .map_err(|e| format!("UsbAdb: tap failed: {e}"))?;
            Ok(())
        });

        match tokio::time::timeout(Duration::from_secs(5), tap_future).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(format!("UsbAdb: tap task failed: {e}")),
            Err(_) => Err("UsbAdb: tap timed out after 5 seconds".to_string()),
        }
    }

    async fn swipe(
        &self,
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        duration: Option<u32>,
    ) -> Result<(), String> {
        let usb_device = Arc::clone(&self.usb_device);
        let duration_ms = duration.unwrap_or(300);

        let swipe_future = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let mut out: Vec<u8> = Vec::new();
            let mut dev = usb_device.blocking_lock();
            let x1s = x1.to_string();
            let y1s = y1.to_string();
            let x2s = x2.to_string();
            let y2s = y2.to_string();
            let durs = duration_ms.to_string();
            dev.shell_command(&["input", "swipe", &x1s, &y1s, &x2s, &y2s, &durs], &mut out)
                .map_err(|e| format!("UsbAdb: swipe failed: {e}"))?;
            Ok(())
        });

        match tokio::time::timeout(Duration::from_secs(10), swipe_future).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(format!("UsbAdb: swipe task failed: {e}")),
            Err(_) => Err("UsbAdb: swipe timed out after 10 seconds".to_string()),
        }
    }

    async fn get_device_ip(&self) -> Result<String, String> {
        Err("UsbAdb: get_device_ip not supported for USB devices".to_string())
    }

    async fn is_human_touching(&self) -> bool {
        let monitor = self.touch_monitor.read().await;
        monitor.is_human_active()
    }

    async fn get_touch_timeout_remaining(&self) -> Option<u64> {
        let monitor = self.touch_monitor.read().await;
        monitor.get_remaining_seconds()
    }

    async fn clear_touch_activity(&self) -> Result<(), String> {
        let mut monitor = self.touch_monitor.write().await;
        monitor.clear_touch_activity();
        Ok(())
    }

    async fn register_touch_activity(&self) -> Result<(), String> {
        let mut monitor = self.touch_monitor.write().await;
        monitor.mark_touch_activity();
        Ok(())
    }

    async fn start_touch_monitoring(&self) -> Result<(), String> {
        // Touch monitoring not implemented for USB yet
        Ok(())
    }

    async fn stop_touch_monitoring(&self) -> Result<(), String> {
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
