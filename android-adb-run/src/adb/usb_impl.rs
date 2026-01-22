use super::error::{AdbError, AdbResult};
use super::types::{AdbClient, Device, TouchActivityMonitor, TouchActivityState, UsbCommand};
use adb_client::{ADBDeviceExt, ADBUSBDevice};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, mpsc};

pub struct UsbAdb {
    device: Device,
    usb_device: Arc<Mutex<ADBUSBDevice>>,
    screen_x: u32,
    screen_y: u32,
    touch_monitor: TouchActivityMonitor,
    monitoring_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,

    // Unified command queue for all USB operations
    usb_queue_tx: mpsc::Sender<UsbCommand>,

    usb_processor_handle: Option<tokio::task::JoinHandle<()>>,
    pub debug_enabled: bool,
}
impl UsbAdb {
    async fn get_screen_size_with(&self) -> AdbResult<(u32, u32)> {
        let screen_size_future = async {
            let mut out: Vec<u8> = Vec::new();
            {
                let mut dev = self.usb_device.lock().await;
                dev.shell_command(&["wm", "size"], &mut out).map_err(|e| {
                    AdbError::ShellCommandFailed {
                        command: "wm size".into(),
                        source: e,
                    }
                })?;
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
            Err(AdbError::ScreenSizeParseFailed)
        };

        tokio::time::timeout(std::time::Duration::from_secs(5), screen_size_future)
            .await
            .map_err(|_| AdbError::Timeout {
                duration: Duration::from_secs(5),
                description: "Screen size detection".into(),
            })?
    }

    #[allow(dead_code)]
    async fn capture_screen_bytes_internal(&self) -> AdbResult<Vec<u8>> {
        let mut dev = self.usb_device.lock().await;

        // Try the faster framebuffer_bytes() method first
        match dev.framebuffer_bytes() {
            Ok(framebuffer_data) => {
                drop(dev); // Release the lock early
                match self.framebuffer_to_png(framebuffer_data).await {
                    Ok(png_data) => return Ok(png_data),
                    Err(e) => {
                        log::warn!(
                            "Framebuffer conversion failed, falling back to screencap: {}",
                            e
                        );
                        // Continue to fallback method below
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "Framebuffer capture failed, falling back to screencap: {}",
                    e
                );
                // Continue to fallback method below
            }
        }

        // Fallback to shell screencap method
        let mut dev = self.usb_device.lock().await;
        let mut out: Vec<u8> = Vec::new();
        dev.shell_command(&["screencap", "-p"], &mut out)
            .map_err(|e| AdbError::ShellCommandFailed {
                command: "screencap -p".into(),
                source: e,
            })?;
        Ok(out)
    }

    #[allow(dead_code)]
    async fn framebuffer_to_png(&self, framebuffer_data: Vec<u8>) -> AdbResult<Vec<u8>> {
        use image::{ImageBuffer, codecs::png::PngEncoder};
        use std::io::Cursor;

        let pixel_count = (self.screen_x * self.screen_y) as usize;
        let data_len = framebuffer_data.len();

        if data_len < pixel_count {
            if framebuffer_data.len() >= 8 && &framebuffer_data[0..8] == b"\x89PNG\r\n\x1a\n" {
                return Ok(framebuffer_data);
            }

            if framebuffer_data.len() >= 2
                && framebuffer_data[0] == 0xFF
                && framebuffer_data[1] == 0xD8
            {
                return self.jpeg_to_png(framebuffer_data).await;
            }

            return Err(AdbError::FramebufferToPngFailed {
                description: format!(
                    "Data appears to be compressed or in unsupported format: {} bytes for {} pixels",
                    data_len, pixel_count
                ),
            });
        }

        if data_len < pixel_count * 2 {
            return Err(AdbError::FramebufferToPngFailed {
                description: format!(
                    "Data too small for raw format: {} bytes for {} pixels (minimum {} for RGB565)",
                    data_len,
                    pixel_count,
                    pixel_count * 2
                ),
            });
        }

        let (header_size, _actual_data_len, bytes_per_pixel) = {
            let mut best_match = (0, 0, 0);
            for header in [0, 12, 16, 20, 24] {
                if header >= data_len {
                    break;
                }
                let test_data_len = data_len - header;
                let bpp = if test_data_len >= pixel_count * 4 {
                    4
                } else {
                    0
                };
                if bpp > 0 {
                    best_match = (header, test_data_len, bpp);
                    break;
                }
            }
            best_match
        };

        let actual_data = &framebuffer_data[header_size..];

        let png_data = match bytes_per_pixel {
            4 => {
                let img = ImageBuffer::<image::Rgba<u8>, _>::from_raw(
                    self.screen_x,
                    self.screen_y,
                    actual_data.to_vec(),
                )
                .ok_or(AdbError::FramebufferToPngFailed {
                    description: "Failed to create RGBA image from data".into(),
                })?;
                let mut data = Vec::new();
                img.write_with_encoder(PngEncoder::new(Cursor::new(&mut data)))
                    .map_err(|e| AdbError::FramebufferToPngFailed {
                        description: format!("Failed to encode RGBA PNG: {}", e),
                    })?;
                data
            }
            _ => {
                return Err(AdbError::FramebufferToPngFailed {
                    description: format!(
                        "Unsupported framebuffer format: {} bytes per pixel",
                        bytes_per_pixel
                    ),
                });
            }
        };

        Ok(png_data)
    }

    #[allow(dead_code)]
    async fn jpeg_to_png(&self, jpeg_data: Vec<u8>) -> AdbResult<Vec<u8>> {
        use image::{ImageFormat, codecs::png::PngEncoder};
        use std::io::Cursor;

        let img =
            image::load_from_memory_with_format(&jpeg_data, ImageFormat::Jpeg).map_err(|e| {
                AdbError::JpegToPngFailed {
                    description: format!("Failed to decode JPEG: {}", e),
                }
            })?;

        let mut data = Vec::new();
        img.write_with_encoder(PngEncoder::new(Cursor::new(&mut data)))
            .map_err(|e| AdbError::JpegToPngFailed {
                description: format!("Failed to encode JPEG as PNG: {}", e),
            })?;
        Ok(data)
    }

    async fn monitor_touch_activity_loop(
        touch_monitor: TouchActivityMonitor,
        _usb_device: Arc<Mutex<ADBUSBDevice>>,
        usb_queue_tx: mpsc::Sender<UsbCommand>,
    ) -> AdbResult<()> {
        // Note: We don't need usb_device anymore since we use the queue for touch polling
        // The touch device path is determined once at startup
        let event_device = "/dev/input/event2".to_string(); // Default, most devices use event2
        Self::stream_touch_events_polling(usb_queue_tx, &event_device, touch_monitor).await
    }

    async fn stream_touch_events_polling(
        usb_queue_tx: mpsc::Sender<UsbCommand>,
        event_device: &str,
        touch_monitor: TouchActivityMonitor,
    ) -> AdbResult<()> {
        // Poll for touch events using the USB command queue
        // Each poll uses "timeout 0.3 getevent -c 1" which blocks for max 300ms
        // Polling every 1 second means we check for touches periodically without
        // overloading the USB command queue with touch check requests
        let poll_interval = Duration::from_secs(1);

        log::info!("Touch monitoring started for device: {}", event_device);

        loop {
            // Check if we should stop monitoring
            if !touch_monitor.read().await.is_monitoring {
                log::info!("Touch monitoring stopped");
                break;
            }

            // Clear expired touch activity
            if touch_monitor.read().await.has_activity_expired() {
                touch_monitor.write().await.last_touch_time = None;
            }

            // Poll for touch events through the USB queue
            let (tx, rx) = tokio::sync::oneshot::channel();
            let send_result = usb_queue_tx
                .send(UsbCommand::CheckTouchEvent {
                    event_device: event_device.to_string(),
                    response_tx: tx,
                })
                .await;

            if send_result.is_err() {
                log::warn!("Touch monitor: USB queue closed");
                break;
            }

            // Wait for the result with a timeout
            match tokio::time::timeout(Duration::from_secs(2), rx).await {
                Ok(Ok(Ok(touch_detected))) => {
                    if touch_detected {
                        log::info!("Human touch detected - marking activity");
                        touch_monitor.write().await.mark_touch_activity();
                    }
                }
                Ok(Ok(Err(e))) => {
                    log::debug!("Touch check failed: {}", e);
                    // Continue monitoring despite errors
                }
                Ok(Err(_)) => {
                    log::warn!("Touch monitor: channel closed");
                    break;
                }
                Err(_) => {
                    log::warn!("Touch check timed out");
                    // Continue monitoring
                }
            }

            // Wait before next poll
            tokio::time::sleep(poll_interval).await;
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn is_touch_event_line(line: &str) -> bool {
        line.contains("ABS_MT")
            || line.contains("BTN_TOUCH")
            || line.contains("BTN_TOOL_FINGER")
            || line.contains("ABS_X")
            || line.contains("ABS_Y")
            || (line.contains("0003") && (line.contains("0035") || line.contains("0036")))
    }

    #[allow(dead_code)] // May be used in future for dynamic device detection
    async fn find_touch_event_device(usb_device: Arc<Mutex<ADBUSBDevice>>) -> AdbResult<String> {
        let mut out = Vec::new();
        usb_device
            .lock()
            .await
            .shell_command(&["getevent", "-p"], &mut out)
            .map_err(|e| AdbError::ShellCommandFailed {
                command: "getevent -p".into(),
                source: e,
            })?;

        let output = String::from_utf8_lossy(&out);
        let mut current_device: Option<String> = None;
        let mut has_touch_events = false;
        let mut best_device: Option<String> = None;

        for line in output.lines() {
            if line.starts_with("add device") {
                if has_touch_events {
                    best_device = current_device.clone();
                }
                if let Some(path_start) = line.find("/dev/input/event") {
                    current_device = Some(line[path_start..].to_string());
                    has_touch_events = false;
                }
            } else if line.contains("0035") || line.contains("0036") {
                has_touch_events = true;
            }
        }
        if has_touch_events {
            best_device = current_device;
        }

        best_device.ok_or(AdbError::NoTouchDeviceFound)
    }
}

impl AdbClient for UsbAdb {
    async fn list_devices() -> AdbResult<Vec<Device>> {
        let list_future = tokio::task::spawn_blocking(|| match adb_client::search_adb_devices() {
            Ok(Some((vendor_id, product_id))) => Ok(vec![Device {
                name: format!("{:04x}:{:04x}", vendor_id, product_id),
                transport_id: None,
            }]),
            Ok(None) => Ok(vec![]),
            Err(e) => Err(AdbError::DeviceEnumerationFailed { source: e }),
        });

        match tokio::time::timeout(Duration::from_secs(2), list_future).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(AdbError::from(e)),
            Err(_) => Err(AdbError::Timeout {
                duration: Duration::from_secs(2),
                description: "Device enumeration".into(),
            }),
        }
    }

    async fn new_with_device(device_name: &str) -> AdbResult<Self> {
        let key_path = homedir::my_home()
            .ok()
            .flatten()
            .map(|home| home.join(".android").join("adbkey"))
            .ok_or(AdbError::HomeDirectoryNotFound)?;

        if !key_path.exists() {
            return Err(AdbError::KeyNotFound { path: key_path });
        }

        let mut usb_device = None;
        let max_attempts = 5;

        for _ in 1..=max_attempts {
            let key_path_clone = key_path.clone();
            let usb_future = tokio::task::spawn_blocking(move || {
                ADBUSBDevice::autodetect_with_custom_private_key(key_path_clone)
            });

            match tokio::time::timeout(Duration::from_secs(10), usb_future).await {
                Ok(Ok(device_result)) => match device_result {
                    Ok(device) => {
                        usb_device = Some(device);
                        break;
                    }
                    Err(e) => {
                        log::warn!("Connection attempt failed: {}. Retrying...", e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                },
                Ok(Err(e)) => return Err(AdbError::from(e)),
                Err(_) => {
                    return Err(AdbError::ConnectionTimeout {
                        duration: Duration::from_secs(10),
                    });
                }
            }
        }

        let mut usb_device = usb_device.ok_or_else(|| AdbError::ConnectionFailed {
            source: adb_client::RustADBError::IOError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No USB devices found after retries",
            )),
        })?;

        // Step 3.5: Validate authentication with timeout
        let auth_timeout = Duration::from_secs(30);
        let auth_start = std::time::Instant::now();
        loop {
            let mut test_output = Vec::new();
            match usb_device.shell_command(&["echo", "test"], &mut test_output) {
                Ok(_) => {
                    let output = String::from_utf8_lossy(&test_output);
                    if output.trim() == "test" {
                        println!("✅ Authenticated");
                        break;
                    } else {
                        println!("⚠️ Unexpected echo response: {}", output.trim());
                    }
                }
                Err(e) => {
                    println!("⚠️ Authorization check failed: {}", e);
                }
            }
            if auth_start.elapsed() > auth_timeout {
                return Err(AdbError::ConnectionValidationTimeout);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let (dummy_tx, _) = mpsc::channel(1);
        let mut tmp = UsbAdb {
            device: Device {
                name: device_name.to_string(),
                transport_id: None,
            },
            debug_enabled: false,
            usb_device: Arc::new(Mutex::new(usb_device)),
            screen_x: 0,
            screen_y: 0,
            touch_monitor: Arc::new(RwLock::new(TouchActivityState::new(30))),
            monitoring_task: Arc::new(Mutex::new(None)),
            usb_queue_tx: dummy_tx,
            usb_processor_handle: None,
        };

        let (sx, sy) = tmp.get_screen_size_with().await?;

        let (tx, mut rx) = mpsc::channel::<UsbCommand>(100);

        let usb_clone = Arc::clone(&tmp.usb_device);

        let screen_x = sx;
        let screen_y = sy;
        let debug_enabled = tmp.debug_enabled;

        // Unified USB command processor - serializes ALL USB operations
        let processor = tokio::spawn(async move {
            println!("🔧 USB command processor started");
            while let Some(cmd) = rx.recv().await {
                let mut dev = usb_clone.lock().await;

                match cmd {
                    UsbCommand::Tap { x, y, response_tx } => {
                        if x > screen_x || y > screen_y {
                            println!("❌ Tap out of bounds: ({},{})", x, y);
                            let _ = response_tx.send(Err(AdbError::TapOutOfBounds { x, y }));
                            continue;
                        }

                        let mut out = Vec::new();
                        let result = match dev.shell_command(
                            &["input", "tap", &x.to_string(), &y.to_string()],
                            &mut out,
                        ) {
                            Ok(_) => {
                                debug_print!(debug_enabled, "✅ Tap executed: ({},{})", x, y);
                                Ok(())
                            }
                            Err(e) => {
                                let err = AdbError::from_adb_error_with_desync_check(
                                    "input tap".into(),
                                    e,
                                );
                                if err.is_protocol_desync() {
                                    eprintln!(
                                        "❌ Tap failed (PROTOCOL DESYNC - reconnection needed): {} ({},{})",
                                        err, x, y
                                    );
                                } else {
                                    eprintln!("❌ Tap failed: {} ({},{})", err, x, y);
                                }
                                Err(err)
                            }
                        };
                        let _ = response_tx.send(result);
                    }

                    UsbCommand::Swipe {
                        x1,
                        y1,
                        x2,
                        y2,
                        duration,
                        response_tx,
                    } => {
                        let duration_ms = duration.unwrap_or(300);
                        let mut out = Vec::new();

                        let result = match dev.shell_command(
                            &[
                                "input",
                                "swipe",
                                &x1.to_string(),
                                &y1.to_string(),
                                &x2.to_string(),
                                &y2.to_string(),
                                &duration_ms.to_string(),
                            ],
                            &mut out,
                        ) {
                            Ok(_) => {
                                println!("✅ Swipe executed");
                                Ok(())
                            }
                            Err(e) => {
                                let err = AdbError::from_adb_error_with_desync_check(
                                    "input swipe".into(),
                                    e,
                                );
                                if err.is_protocol_desync() {
                                    eprintln!(
                                        "❌ Swipe failed (PROTOCOL DESYNC - reconnection needed): {}",
                                        err
                                    );
                                } else {
                                    eprintln!("❌ Swipe failed: {}", err);
                                }
                                Err(err)
                            }
                        };
                        let _ = response_tx.send(result);
                    }

                    UsbCommand::Screenshot { response_tx } => {
                        let result = match dev.framebuffer_bytes() {
                            Ok(data) => Ok(data),
                            Err(fb_err) => {
                                // Framebuffer failed, try screencap fallback
                                let mut out = Vec::new();
                                match dev.shell_command(&["screencap", "-p"], &mut out) {
                                    Ok(_) => Ok(out),
                                    Err(e) => {
                                        let err = AdbError::from_adb_error_with_desync_check(
                                            "screencap -p".into(),
                                            e,
                                        );
                                        if err.is_protocol_desync() {
                                            eprintln!(
                                                "❌ Screenshot failed (PROTOCOL DESYNC - reconnection needed): {}",
                                                err
                                            );
                                        }
                                        // Also check if framebuffer error was a desync
                                        let fb_err_str = fb_err.to_string();
                                        if fb_err_str.contains("CLSE")
                                            || fb_err_str.contains("no write endpoint")
                                        {
                                            Err(AdbError::ProtocolDesync {
                                                description: "Framebuffer and screencap both failed with protocol errors".to_string(),
                                            })
                                        } else {
                                            Err(err)
                                        }
                                    }
                                }
                            }
                        };
                        let _ = response_tx.send(result);
                    }

                    UsbCommand::CheckTouchEvent {
                        event_device,
                        response_tx,
                    } => {
                        // Use Android's timeout command with getevent for non-blocking poll
                        // timeout 0.3 getevent -c 1 /dev/input/eventX
                        // Returns output if touch detected, empty if timeout
                        let mut out = Vec::new();
                        let result = dev
                            .shell_command(
                                &["timeout", "0.3", "getevent", "-c", "1", &event_device],
                                &mut out,
                            )
                            .map(|_| {
                                // Touch detected if we got any output
                                let output = String::from_utf8_lossy(&out);
                                let has_event = !output.trim().is_empty();
                                if has_event {
                                    log::debug!("Touch event detected: {}", output.trim());
                                }
                                has_event
                            })
                            .map_err(|e| AdbError::ShellCommandFailed {
                                command: format!("timeout getevent {}", event_device),
                                source: e,
                            });
                        let _ = response_tx.send(result);
                    }
                }
                drop(dev);
            }
        });

        tmp.usb_queue_tx = tx;
        tmp.usb_processor_handle = Some(processor);

        Ok(UsbAdb {
            device: tmp.device,
            usb_device: tmp.usb_device,
            screen_x: sx,
            screen_y: sy,
            touch_monitor: tmp.touch_monitor,
            monitoring_task: tmp.monitoring_task,
            usb_queue_tx: tmp.usb_queue_tx,
            usb_processor_handle: tmp.usb_processor_handle,
            debug_enabled: tmp.debug_enabled,
        })
    }

    async fn screen_capture_bytes(&self) -> AdbResult<Vec<u8>> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.usb_queue_tx
            .send(UsbCommand::Screenshot { response_tx: tx })
            .await
            .map_err(|_| AdbError::ChannelClosed)?;

        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(AdbError::ChannelClosed),
            Err(_) => Err(AdbError::Timeout {
                duration: Duration::from_secs(30),
                description: "Screenshot capture".into(),
            }),
        }
    }

    async fn tap(&self, x: u32, y: u32) -> AdbResult<()> {
        if x > self.screen_x || y > self.screen_y {
            return Err(AdbError::TapOutOfBounds { x, y });
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.usb_queue_tx
            .send(UsbCommand::Tap {
                x,
                y,
                response_tx: tx,
            })
            .await
            .map_err(|_| AdbError::ChannelClosed)?;

        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(AdbError::ChannelClosed),
            Err(_) => Err(AdbError::Timeout {
                duration: Duration::from_secs(30),
                description: "Tap execution".into(),
            }),
        }
    }

    async fn swipe(
        &self,
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        duration: Option<u32>,
    ) -> AdbResult<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.usb_queue_tx
            .send(UsbCommand::Swipe {
                x1,
                y1,
                x2,
                y2,
                duration,
                response_tx: tx,
            })
            .await
            .map_err(|_| AdbError::ChannelClosed)?;

        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(AdbError::ChannelClosed),
            Err(_) => Err(AdbError::Timeout {
                duration: Duration::from_secs(30),
                description: "Swipe execution".into(),
            }),
        }
    }

    async fn get_device_ip(&self) -> AdbResult<String> {
        Err(AdbError::UnsupportedUsbOperation {
            operation: "get_device_ip".into(),
        })
    }

    async fn is_human_touching(&self) -> bool {
        self.touch_monitor.read().await.is_human_active()
    }

    async fn get_touch_timeout_remaining(&self) -> Option<u64> {
        self.touch_monitor.read().await.get_remaining_seconds()
    }

    async fn clear_touch_activity(&self) -> AdbResult<()> {
        self.touch_monitor.write().await.clear_touch_activity();
        Ok(())
    }

    async fn register_touch_activity(&self) -> AdbResult<()> {
        self.touch_monitor.write().await.mark_touch_activity();
        Ok(())
    }

    async fn start_touch_monitoring(&self) -> AdbResult<()> {
        let mut monitor = self.touch_monitor.write().await;
        if monitor.is_monitoring {
            return Ok(());
        }
        monitor.is_monitoring = true;
        drop(monitor);

        let touch_monitor = Arc::clone(&self.touch_monitor);
        let usb_device = Arc::clone(&self.usb_device);
        let usb_queue_tx = self.usb_queue_tx.clone();

        let task = tokio::spawn(async move {
            if let Err(e) =
                Self::monitor_touch_activity_loop(touch_monitor.clone(), usb_device, usb_queue_tx)
                    .await
            {
                log::error!("Touch monitoring ended: {}", e);
            }
            touch_monitor.write().await.is_monitoring = false;
        });

        *self.monitoring_task.lock().await = Some(task);
        Ok(())
    }

    async fn stop_touch_monitoring(&self) -> AdbResult<()> {
        self.touch_monitor.write().await.is_monitoring = false;
        if let Some(task) = self.monitoring_task.lock().await.take() {
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

// Additional methods for UsbAdb (not part of AdbClient trait)
impl UsbAdb {
    /// Gracefully shutdown the USB processor task and release resources
    pub async fn shutdown(&mut self) -> AdbResult<()> {
        // Stop touch monitoring
        self.stop_touch_monitoring().await?;

        // Close the channel by creating a new empty sender (dropping the original)
        // This signals the processor task to exit
        let (new_tx, _) = tokio::sync::mpsc::channel(1);
        self.usb_queue_tx = new_tx;

        // Abort the processor task if it exists
        if let Some(handle) = self.usb_processor_handle.take() {
            handle.abort();
            // Give it a moment to clean up
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }
}
