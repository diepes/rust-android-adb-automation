use super::types::{AdbClient, Device, TouchActivityMonitor, TouchActivityState};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};

pub struct AdbShell {
    pub device: Device,
    pub transport_id: u32,
    pub screen_x: u32,
    pub screen_y: u32,
    touch_monitor: TouchActivityMonitor,
    monitoring_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl AdbShell {
    fn ensure_adb_available() -> Result<(), String> {
        match std::process::Command::new("adb").arg("version").output() {
            Ok(out) => {
                if !out.status.success() {
                    return Err(format!(
                        "'adb' command found but returned non-zero ({}). Ensure Android Platform Tools are properly installed, or restart with --impl=rust to use the pure Rust backend.",
                        out.status
                    ));
                }
                Ok(())
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Err("'adb' binary not found in PATH. Install Android Platform Tools (https://developer.android.com/tools/adb) or add 'adb' to PATH. Alternatively run with --impl=rust (pure Rust backend).".to_string())
                } else {
                    Err(format!(
                        "Failed to invoke 'adb': {e}. Verify installation or switch to --impl=rust."
                    ))
                }
            }
        }
    }

    pub async fn new(transport_id: Option<&str>) -> Result<Self, String> {
        // Provide early backend guidance if adb unavailable
        Self::ensure_adb_available()?;
        let devices = Self::list_devices().await?;
        if devices.is_empty() {
            return Err("No devices available (shell backend). Connect a device or use --impl=rust for pure Rust backend.".to_string());
        }
        let device = match transport_id {
            Some(tid) => devices
                .into_iter()
                .find(|d| d.transport_id.as_deref() == Some(tid)),
            None => devices.into_iter().next(),
        }
        .ok_or_else(|| "Device with specified transport_id not found".to_string())?;
        let transport_id = device
            .transport_id
            .as_ref()
            .ok_or_else(|| "Device missing transport_id".to_string())?
            .parse::<u32>()
            .map_err(|_| "Invalid transport_id format".to_string())?;
        let (screen_x, screen_y) = Self::get_screen_size().await?;
        Ok(Self {
            device,
            transport_id,
            screen_x,
            screen_y,
            touch_monitor: Arc::new(RwLock::new(TouchActivityState::new(30))), // 30 second timeout
            monitoring_task: Arc::new(Mutex::new(None)),
        })
    }

    async fn get_screen_size() -> Result<(u32, u32), String> {
        Self::ensure_adb_available()?;
        let output = Command::new("adb")
            .arg("shell")
            .arg("wm")
            .arg("size")
            .output()
            .await
            .map_err(|e| format!("Failed to run adb shell wm size: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "adb shell wm size failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Self::parse_screen_size(&stdout)
    }

    fn parse_screen_size(stdout: &str) -> Result<(u32, u32), String> {
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
        Err("Could not parse screen size".into())
    }

    pub async fn new_with_device(device_name: &str) -> Result<Self, String> {
        let devices = Self::list_devices().await?;
        if let Some(device) = devices.iter().find(|d| d.name == device_name) {
            return Self::new(device.transport_id.as_deref()).await;
        }
        let output = Command::new("adb")
            .arg("connect")
            .arg(device_name)
            .output()
            .await
            .map_err(|e| format!("Failed to run adb connect: {e}"))?;
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        if !output.status.success()
            || stdout_str.contains("Connection refused")
            || stderr_str.contains("Connection refused")
        {
            return Err(format!(
                "adb connect failed: Out:{stdout_str}\nErr:{stderr_str}\n Try: 'adb tcpip 5555'"
            ));
        }
        let devices = Self::list_devices().await?;
        if let Some(device) = devices.iter().find(|d| d.name == device_name) {
            return Self::new(device.transport_id.as_deref()).await;
        }
        Err(format!("Device '{device_name}' not found after connect"))
    }

    pub fn parse_devices(output: &str) -> Vec<Device> {
        output
            .lines()
            .skip(1)
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1] == "device" {
                    let name = parts[0].to_string();
                    let transport_id = line.split_whitespace().find_map(|part| {
                        if part.starts_with("transport_id:") {
                            Some(part.trim_start_matches("transport_id:").to_string())
                        } else {
                            None
                        }
                    });
                    Some(Device { name, transport_id })
                } else {
                    None
                }
            })
            .collect()
    }

    pub async fn list_devices() -> Result<Vec<Device>, String> {
        Self::ensure_adb_available()?;
        let output = Command::new("adb")
            .arg("devices")
            .arg("-l")
            .output()
            .await
            .map_err(|e| format!("Failed to execute adb: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "adb devices failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(Self::parse_devices(&stdout))
    }

    pub async fn screen_capture(&self, output_path: &str) -> Result<(), String> {
        let cap = <AdbShell as AdbClient>::screen_capture(self).await?;
        tokio::fs::write(output_path, &cap.bytes)
            .await
            .map_err(|e| format!("Failed to write PNG file: {e}"))?;
        Ok(())
    }

    pub async fn capture_screen_bytes_internal(&self) -> Result<Vec<u8>, String> {
        Self::ensure_adb_available()?;
        let mut cmd = Command::new("adb");
        cmd.arg("-t").arg(self.transport_id.to_string());
        let output = cmd
            .arg("exec-out")
            .arg("screencap")
            .arg("-p")
            .output()
            .await
            .map_err(|e| format!("Failed to run adb screencap: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "adb screencap failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(output.stdout)
    }

    pub async fn tap(&self, x: u32, y: u32) -> Result<(), String> {
        Self::ensure_adb_available()?;
        if x > self.screen_x || y > self.screen_y {
            return Err(format!("Coordinates out of bounds x={x} y={y}"));
        }
        let output = Command::new("adb")
            .arg("shell")
            .arg("input")
            .arg("tap")
            .arg(x.to_string())
            .arg(y.to_string())
            .output()
            .await
            .map_err(|e| format!("Failed to run adb shell input tap: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "adb tap failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }

    pub async fn swipe(
        &self,
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        duration: Option<u32>,
    ) -> Result<(), String> {
        Self::ensure_adb_available()?;
        for &(x, y) in &[(x1, y1), (x2, y2)] {
            if x > self.screen_x || y > self.screen_y {
                return Err("Swipe coordinates out of bounds".into());
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
            .map_err(|e| format!("Failed to run adb shell input swipe: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "adb swipe failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }

    pub async fn get_device_ip(&self) -> Result<String, String> {
        Self::ensure_adb_available()?;

        let output = Command::new("adb")
            .arg("shell")
            .arg("ip")
            .arg("route")
            .output()
            .await
            .map_err(|e| format!("Failed to run adb shell ip route: {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "adb shell ip route failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let ip_route_output = String::from_utf8_lossy(&output.stdout);

        // Parse the output to extract the IP from field 9 (like awk '{print $9}')
        for line in ip_route_output.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 9 {
                let potential_ip = fields[8]; // 0-indexed, so field 9 is index 8

                // Validate that it looks like an IP address
                if potential_ip.split('.').count() == 4
                    && potential_ip.chars().all(|c| c.is_ascii_digit() || c == '.')
                {
                    return Ok(potential_ip.to_string());
                }
            }
        }

        Err("No valid IP address found in routing table".to_string())
    }

    async fn monitor_touch_activity_loop(
        touch_monitor: TouchActivityMonitor,
        device_name: String,
        transport_id: u32,
    ) -> Result<(), String> {
        Self::ensure_adb_available()?;

        // Find the correct touch input device
        let event_device = Self::find_touch_event_device(&device_name, transport_id).await?;

        if crate::gui::dioxus_app::is_debug_mode() {
            println!(
                "🔍 Starting continuous touch monitoring on device: {}",
                event_device
            );
        }

        // Start continuous event streaming
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
            match Self::stream_touch_events(transport_id, &event_device, touch_monitor.clone())
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

    // Stream touch events continuously using external adb process
    async fn stream_touch_events(
        transport_id: u32,
        event_device: &str,
        touch_monitor: TouchActivityMonitor,
    ) -> Result<(), String> {
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::process::Command;

        if crate::gui::dioxus_app::is_debug_mode() {
            println!("📡 Opening getevent stream for {}", event_device);
        }

        // Create a continuous getevent process
        let mut cmd = Command::new("adb");
        cmd.arg("-t")
            .arg(transport_id.to_string())
            .arg("shell")
            .arg("getevent")
            .arg("-lt")
            .arg(event_device);

        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start getevent process: {}", e))?;

        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;

        let mut reader = BufReader::new(stdout).lines();

        // Spawn a timeout checker task
        let timeout_monitor = touch_monitor.clone();
        let timeout_task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;

                let monitor = timeout_monitor.read().await;
                if !monitor.is_monitoring {
                    break;
                }

                // Check if activity has expired
                if monitor.has_activity_expired() && monitor.last_touch_time.is_some() {
                    drop(monitor);
                    let mut monitor = timeout_monitor.write().await;
                    monitor.last_touch_time = None;
                    if crate::gui::dioxus_app::is_debug_mode() {
                        println!("⏰ Touch activity timeout - marking as inactive");
                    }
                }
            }
        });

        // Main event reading loop
        loop {
            tokio::select! {
                line_result = reader.next_line() => {
                    match line_result {
                        Ok(Some(line)) => {
                            // Check if this line represents a touch event
                            if Self::is_touch_event_line(&line) {
                                {
                                    let mut monitor = touch_monitor.write().await;
                                    monitor.update_activity();
                                }

                                if crate::gui::dioxus_app::is_debug_mode() {
                                    println!("👆 Touch event: {}", line.trim());
                                }
                            }
                        }
                        Ok(None) => {
                            // EOF - process ended
                            if crate::gui::dioxus_app::is_debug_mode() {
                                println!("📱 getevent stream ended");
                            }
                            break;
                        }
                        Err(e) => {
                            if crate::gui::dioxus_app::is_debug_mode() {
                                eprintln!("📱 Error reading getevent line: {}", e);
                            }
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    // Periodic check if monitoring should continue
                    let monitor = touch_monitor.read().await;
                    if !monitor.is_monitoring {
                        if crate::gui::dioxus_app::is_debug_mode() {
                            println!("🛑 Stopping touch event stream");
                        }
                        break;
                    }
                }
            }
        }

        // Clean up
        let _ = child.kill().await;
        timeout_task.abort();

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
        _device_name: &str,
        transport_id: u32,
    ) -> Result<String, String> {
        // Use getevent -p to get detailed device information
        let output = Command::new("adb")
            .arg("-t")
            .arg(transport_id.to_string())
            .arg("shell")
            .arg("getevent")
            .arg("-p")
            .output()
            .await
            .map_err(|e| format!("Failed to run getevent -p: {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to run getevent -p: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if crate::gui::dioxus_app::is_debug_mode() {
            println!("🔍 Parsing getevent -p output for touch devices...");
        }

        // Parse the output to find touch-capable devices
        let mut current_device: Option<String> = None;
        let mut current_name = String::new();
        let mut has_touch_events = false;
        let mut best_device: Option<String> = None;
        let mut best_score = 0;

        for line in stdout.lines() {
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
}

impl AdbClient for AdbShell {
    async fn list_devices() -> Result<Vec<Device>, String> {
        Self::list_devices().await
    }
    async fn new_with_device(device_name: &str) -> Result<Self, String> {
        Self::new_with_device(device_name).await
    }
    async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String> {
        self.capture_screen_bytes_internal().await
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
    async fn get_device_ip(&self) -> Result<String, String> {
        self.get_device_ip().await
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
        let device_name = self.device.name.clone();
        let transport_id = self.transport_id;

        // Start background monitoring task
        let task = tokio::spawn(async move {
            if let Err(e) =
                Self::monitor_touch_activity_loop(touch_monitor.clone(), device_name, transport_id)
                    .await
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
        Some(self.transport_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_devices_basic() {
        let adb_output = "List of devices attached\nabc123 device transport_id:5\n";
        let devs = AdbShell::parse_devices(adb_output);
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].name, "abc123");
        assert_eq!(devs[0].transport_id, Some("5".to_string()));
    }

    #[test]
    fn test_parse_devices_multiple() {
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\noneplus6:5555          device product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:3\n";
        let devices = AdbShell::parse_devices(adb_output);
        assert_eq!(
            devices,
            vec![
                Device {
                    name: "1d36d8f1".to_string(),
                    transport_id: Some("2".to_string())
                },
                Device {
                    name: "oneplus6:5555".to_string(),
                    transport_id: Some("3".to_string())
                },
            ]
        );
    }

    #[test]
    fn test_parse_devices_single() {
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\n";
        let devices = AdbShell::parse_devices(adb_output);
        assert_eq!(
            devices,
            vec![Device {
                name: "1d36d8f1".to_string(),
                transport_id: Some("2".to_string())
            }]
        );
    }

    #[test]
    fn test_list_devices_mock() {
        let adb_output = "List of devices attached\n1d36d8f1               device usb:1-4 product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:2\noneplus6:5555          device product:OnePlus6 model:ONEPLUS_A6000 device:OnePlus6 transport_id:3\n";
        let devices = AdbShell::parse_devices(adb_output);
        assert_eq!(
            devices,
            vec![
                Device {
                    name: "1d36d8f1".to_string(),
                    transport_id: Some("2".to_string())
                },
                Device {
                    name: "oneplus6:5555".to_string(),
                    transport_id: Some("3".to_string())
                },
            ]
        );
    }
}
