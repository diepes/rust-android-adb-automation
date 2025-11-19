// https://crates.io/crates/adb_client
use super::types::{AdbClient, Device, TouchActivityMonitor, TouchActivityState};
use adb_client::{ADBDeviceExt, ADBServer, ADBServerDevice};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use crate::game_automation::fsm::is_disconnect_error;

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
        // Find the correct touch input device
        let event_device = Self::find_touch_event_device(server_device.clone()).await?;

        if crate::gui::dioxus_app::is_debug_mode() {
            println!(
                "🔍 Starting continuous touch monitoring on device: {}",
                event_device
            );
        }

        // Start a long-running getevent process to continuously stream touch events
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

            // Start continuous event streaming
            match Self::stream_touch_events(
                server_device.clone(),
                &event_device,
                touch_monitor.clone(),
            )
            .await
            {
                Ok(_) => {
                    if crate::gui::dioxus_app::is_debug_mode() {
                        println!("📱 Touch event stream ended, restarting...");
                    }
                }
                Err(e) => {
                    if crate::gui::dioxus_app::is_debug_mode() {
                        eprintln!("⚠️ Touch monitoring error: {}, retrying in 2s...", e);
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }

        Ok(())
    }

    /// Stream touch events continuously using real shell streaming
    /// 
    /// This implementation uses the shell() method with reader/writer to create
    /// a persistent shell session that continuously streams getevent output.
    /// This is much more efficient than running repeated timeout commands.
    async fn stream_touch_events(
        server_device: Arc<Mutex<ADBServerDevice>>,
        event_device: &str,
        touch_monitor: TouchActivityMonitor,
    ) -> Result<(), String> {
        if crate::gui::dioxus_app::is_debug_mode() {
            println!(
                "📡 Starting real-time getevent stream for {}",
                event_device
            );
        }

        // Try the streaming approach first, fall back to polling if it fails
        match Self::stream_touch_events_with_shell_streaming(
            server_device.clone(),
            event_device,
            touch_monitor.clone(),
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                if crate::gui::dioxus_app::is_debug_mode() {
                    eprintln!("⚠️ Shell streaming failed: {}, falling back to polling", e);
                }
                Self::stream_touch_events_polling(server_device, event_device, touch_monitor).await
            }
        }
    }

    /// Real streaming implementation using shell() with reader/writer
    /// 
    /// The shell() method signature from adb_client is:
    /// fn shell(&mut self, reader: &mut dyn Read, writer: Box<dyn Write + Send>) -> Result<()>
    /// 
    /// Where:
    /// - reader: receives input commands to send to the shell
    /// - writer: receives output from the shell
    async fn stream_touch_events_with_shell_streaming(
        server_device: Arc<Mutex<ADBServerDevice>>,
        event_device: &str,
        touch_monitor: TouchActivityMonitor,
    ) -> Result<(), String> {
        use std::io::{Cursor, Write};
        use std::sync::mpsc;
        
        if crate::gui::dioxus_app::is_debug_mode() {
            println!("🔄 Starting shell streaming for getevent on {}", event_device);
        }

        let event_device = event_device.to_string();
        let touch_monitor_clone = Arc::clone(&touch_monitor);
        
        // Create a channel to receive output lines
        let (tx, rx) = mpsc::channel::<String>();
        
        // Prepare the command to send to the shell
        let command = format!("getevent -lt {}\n", event_device);
        
        // Spawn a blocking task to handle the shell I/O
        let handle = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let mut dev = server_device.blocking_lock();
            
            // Create a reader with the command we want to execute
            let mut command_reader = Cursor::new(command.as_bytes());
            
            // Create a writer that will capture the output and send it through the channel
            struct ChannelWriter {
                tx: mpsc::Sender<String>,
                buffer: Vec<u8>,
            }
            
            impl Write for ChannelWriter {
                fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                    self.buffer.extend_from_slice(buf);
                    
                    // Process complete lines
                    while let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
                        let line_bytes = self.buffer.drain(..=pos).collect::<Vec<u8>>();
                        if let Ok(line) = String::from_utf8(line_bytes) {
                            let _ = self.tx.send(line);
                        }
                    }
                    
                    Ok(buf.len())
                }
                
                fn flush(&mut self) -> std::io::Result<()> {
                    // Flush any remaining data
                    if !self.buffer.is_empty() {
                        if let Ok(line) = String::from_utf8(self.buffer.clone()) {
                            let _ = self.tx.send(line);
                        }
                        self.buffer.clear();
                    }
                    Ok(())
                }
            }
            
            let channel_writer = ChannelWriter {
                tx: tx.clone(),
                buffer: Vec::new(),
            };
            
            let boxed_writer: Box<dyn Write + Send> = Box::new(channel_writer);
            
            // Call the shell method
            match dev.shell(&mut command_reader, boxed_writer) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Shell command failed: {}", e))
            }
        });

        // In a separate task, process the lines from the channel
        let process_handle = tokio::spawn(async move {
            while let Ok(line) = rx.recv() {
                // Check if monitoring should continue
                {
                    let monitor = touch_monitor_clone.read().await;
                    if !monitor.is_monitoring {
                        break;
                    }
                }
                
                // Check if this is a touch event
                if Self::is_touch_event_line(&line) {
                    let mut monitor = touch_monitor_clone.write().await;
                    monitor.update_activity();
                    
                    if crate::gui::dioxus_app::is_debug_mode() {
                        println!("👆 Touch event: {}", line.trim());
                    }
                }
            }
        });

        // Wait for the shell task to complete
        match handle.await {
            Ok(result) => {
                // Also wait for processing to finish
                let _ = process_handle.await;
                result
            },
            Err(e) => Err(format!("Task join error: {}", e)),
        }
    }

    /// Optimized polling fallback method
    async fn stream_touch_events_polling(
        server_device: Arc<Mutex<ADBServerDevice>>,
        event_device: &str,
        touch_monitor: TouchActivityMonitor,
    ) -> Result<(), String> {
        if crate::gui::dioxus_app::is_debug_mode() {
            println!("📊 Using optimized polling for touch detection on {}", event_device);
        }

        let check_interval = Duration::from_millis(100); // Faster polling
        let mut consecutive_quiet_periods = 0;

        loop {
            // Check if monitoring should continue
            {
                let monitor = touch_monitor.read().await;
                if !monitor.is_monitoring {
                    break;
                }

                // Check for timeout
                if monitor.has_activity_expired() && monitor.last_touch_time.is_some() {
                    drop(monitor);
                    let mut monitor = touch_monitor.write().await;
                    monitor.last_touch_time = None;
                    if crate::gui::dioxus_app::is_debug_mode() {
                        println!("⏰ Touch activity timeout - marking as inactive");
                    }
                }
            }

            // Use shorter timeout for better responsiveness
            let mut out: Vec<u8> = Vec::new();
            {
                let mut dev = server_device.lock().await;
                let command = format!(
                    "timeout 0.2s getevent -lt {} 2>/dev/null || true",
                    event_device
                );

                if let Ok(_) = dev.shell_command(&["sh", "-c", &command], &mut out) {
                    let output = String::from_utf8_lossy(&out);

                    // Check for touch events in the output
                    let has_touch_events =
                        output.lines().any(|line| Self::is_touch_event_line(line));

                    if has_touch_events {
                        {
                            let mut monitor = touch_monitor.write().await;
                            monitor.update_activity();
                        }

                        consecutive_quiet_periods = 0;

                        if crate::gui::dioxus_app::is_debug_mode() {
                            let event_count = output.lines().count();
                            println!(
                                "👆 Touch activity detected - {} events in 0.2s window",
                                event_count
                            );
                        }
                    } else {
                        consecutive_quiet_periods += 1;

                        // Gradually increase check interval during quiet periods
                        if consecutive_quiet_periods > 20 {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                }
            }

            // Wait before next check
            tokio::time::sleep(check_interval).await;
        }

        Ok(())
    }

    // Check if a getevent line represents a touch event
    fn is_touch_event_line(line: &str) -> bool {
        // Look for touch-related events in getevent output
        let is_touch = line.contains("ABS_MT") ||           // Multi-touch absolute events
            line.contains("BTN_TOUCH") ||        // Touch button events  
            line.contains("BTN_TOOL_FINGER") ||  // Finger tool events
            line.contains("ABS_X") ||            // X coordinate
            line.contains("ABS_Y") ||            // Y coordinate
            (line.contains("0003") && (line.contains("0035") || line.contains("0036"))); // Raw coordinate events

        // Add debug logging to see what events we're detecting
        if is_touch && crate::gui::dioxus_app::is_debug_mode() {
            println!("🔍 Touch event detected: {}", line.trim());
        }

        is_touch
    }

    async fn find_touch_event_device(
        server_device: Arc<Mutex<ADBServerDevice>>,
    ) -> Result<String, String> {
        let mut out: Vec<u8> = Vec::new();
        let mut dev = server_device.lock().await;

        // Use getevent -p to get detailed device information
        dev.shell_command(&["getevent", "-p"], &mut out)
            .map_err(|e| format!("Failed to run getevent -p: {e}"))?;

        let output = String::from_utf8_lossy(&out);
        if crate::gui::dioxus_app::is_debug_mode() {
            println!("🔍 Parsing getevent -p output for touch devices...");
        }

        // Parse the output to find touch-capable devices
        let mut current_device: Option<String> = None;
        let mut current_name = String::new();
        let mut has_touch_events = false;
        let mut best_device: Option<String> = None;
        let mut best_score = 0;

        for line in output.lines() {
            let line = line.trim();

            // New device declaration: "add device N: /dev/input/eventX"
            if line.starts_with("add device") && line.contains("/dev/input/event") {
                // Save previous device if it was touch-capable
                if let Some(ref device) = current_device {
                    if has_touch_events {
                        let score = Self::score_touch_device(&current_name);
                        if crate::gui::dioxus_app::is_debug_mode() {
                            println!(
                                "  📱 Found touch device: {} (name: '{}', score: {})",
                                device, current_name, score
                            );
                        }
                        if score > best_score {
                            best_device = Some(device.clone());
                            best_score = score;
                        }
                    }
                }

                // Extract device path
                if let Some(path_start) = line.find("/dev/input/event") {
                    current_device = Some(line[path_start..].to_string());
                    current_name.clear();
                    has_touch_events = false;
                }
            }
            // Device name: '  name:     "device_name"'
            else if line.starts_with("name:") {
                if let Some(name_start) = line.find('"') {
                    if let Some(name_end) = line.rfind('"') {
                        if name_start < name_end {
                            current_name = line[name_start + 1..name_end].to_string();
                        }
                    }
                }
            }
            // Look for touch-related ABS events
            else if line.contains("ABS (0003)") || line.contains("0035") || line.contains("0036")
            {
                // ABS events with coordinates 0035 (ABS_MT_POSITION_X) or 0036 (ABS_MT_POSITION_Y)
                has_touch_events = true;
            }
        }

        // Check the last device
        if let Some(ref device) = current_device {
            if has_touch_events {
                let score = Self::score_touch_device(&current_name);
                if crate::gui::dioxus_app::is_debug_mode() {
                    println!(
                        "  📱 Found touch device: {} (name: '{}', score: {})",
                        device, current_name, score
                    );
                }
                if score > best_score {
                    best_device = Some(device.clone());
                    best_score = score;
                }
            }
        }

        match best_device {
            Some(device) => {
                if crate::gui::dioxus_app::is_debug_mode() {
                    println!(
                        "✅ Selected touch device: {} (score: {})",
                        device, best_score
                    );
                }
                Ok(device)
            }
            None => Err("No touch-capable input devices found".to_string()),
        }
    }

    // Score touch devices to pick the best one (higher score = better)
    fn score_touch_device(device_name: &str) -> i32 {
        let name_lower = device_name.to_lowercase();
        let mut score = 0;

        // Prioritize known touchscreen vendors
        if name_lower.contains("synaptics") {
            score += 100;
        }
        if name_lower.contains("atmel") {
            score += 90;
        }
        if name_lower.contains("goodix") {
            score += 90;
        }
        if name_lower.contains("focaltech") {
            score += 90;
        }
        if name_lower.contains("ilitek") {
            score += 90;
        }
        if name_lower.contains("cypress") {
            score += 80;
        }
        if name_lower.contains("elan") {
            score += 80;
        }

        // Generic touchscreen indicators
        if name_lower.contains("touch") {
            score += 50;
        }
        if name_lower.contains("screen") {
            score += 40;
        }
        if name_lower.contains("panel") {
            score += 30;
        }
        if name_lower.contains("ts") {
            score += 20;
        } // touchscreen abbreviation

        // Avoid non-touch devices
        if name_lower.contains("button") {
            score -= 50;
        }
        if name_lower.contains("key") {
            score -= 30;
        }
        if name_lower.contains("jack") {
            score -= 50;
        }
        if name_lower.contains("audio") {
            score -= 50;
        }
        if name_lower.contains("gpio") {
            score -= 30;
        }

        score
    }

    /// Connect to the first available device
    pub async fn connect_first() -> Result<Self, String> {
        let devices = Self::list_devices().await?;
        let first = devices
            .into_iter()
            .next()
            .ok_or_else(|| "No devices found".to_string())?;
        Self::new_with_device(&first.name).await
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
        // Add 10-second timeout to detect USB disconnect
        let capture_future = self.capture_screen_bytes_internal();
        
        match tokio::time::timeout(Duration::from_secs(10), capture_future).await {
            Ok(result) => result,
            Err(_) => Err("RustAdb: screenshot capture timed out after 10 seconds (device may be disconnected)".to_string()),
        }
    }

    async fn tap(&self, x: u32, y: u32) -> Result<(), String> {
        // Check bounds before attempting tap
        if x > self.screen_x || y > self.screen_y {
            return Err(format!("RustAdb: tap out of bounds x={x} y={y}"));
        }

        // Clone Arc for move into spawn_blocking
        let server_device = Arc::clone(&self.server_device);
        
        // Wrap the blocking shell_command in spawn_blocking so timeout can work
        let tap_future = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let mut out: Vec<u8> = Vec::new();
            // This blocks until we get the lock, then blocks on shell_command
            let mut dev = server_device.blocking_lock();
            let xs = x.to_string();
            let ys = y.to_string();
            dev.shell_command(&["input", "tap", &xs, &ys], &mut out)
                .map_err(|e| format!("RustAdb: tap failed: {e}"))?;
            Ok(())
        });

        // Timeout wraps the spawn_blocking task
        match tokio::time::timeout(Duration::from_secs(5), tap_future).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(format!("RustAdb: tap task failed: {e}")),
            Err(_) => Err("RustAdb: tap timed out after 5 seconds (device may be disconnected)".to_string()),
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
        // Check bounds before attempting swipe
        for &(x, y) in &[(x1, y1), (x2, y2)] {
            if x > self.screen_x || y > self.screen_y {
                return Err("RustAdb: swipe out of bounds".into());
            }
        }

        // Clone Arc for move into spawn_blocking
        let server_device = Arc::clone(&self.server_device);
        
        // Wrap the blocking shell_command in spawn_blocking so timeout can work
        let swipe_future = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let mut out: Vec<u8> = Vec::new();
            let mut dev = server_device.blocking_lock();
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
                .map_err(|e| {
                    if is_disconnect_error(&e.to_string()) {
                        return "RustAdb: device disconnected".into();
                    }
                    format!("RustAdb: swipe failed: {e}")
                })?;
            Ok(())
        });

        // Timeout wraps the spawn_blocking task
        match tokio::time::timeout(Duration::from_secs(5), swipe_future).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(format!("RustAdb: swipe task failed: {e}")),
            Err(_) => Err("RustAdb: swipe timed out after 5 seconds (device may be disconnected)".to_string()),
        }
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
            if let Err(e) =
                Self::monitor_touch_activity_loop(touch_monitor.clone(), server_device).await
            {
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
