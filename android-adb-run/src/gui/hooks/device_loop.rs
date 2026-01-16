use crate::adb::{AdbBackend, AdbClient};
use crate::gui::hooks::types::*;
use crate::gui::util::base64_encode;
use dioxus::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

/// Initializes device connection loop that discovers and connects to Android devices
/// Uses grouped signal structs for cleaner function signature
/// Also monitors connection and handles reconnection when device disconnects
pub fn use_device_loop(
    mut screenshot: ScreenshotSignals,
    mut device: DeviceSignals,
    mut shared_adb_client: SharedAdbClient,
    mut force_update: Signal<u32>,
) {
    use_future(move || async move {
        loop {
            // === DISCOVERY PHASE ===
            device.status.set("🔍 Looking for devices...".to_string());
            let devices = match AdbBackend::list_devices().await {
                Ok(devices) if !devices.is_empty() => devices,
                Ok(_) => {
                    for seconds in (1..=5).rev() {
                        device.status.set(format!(
                            "🔌 No Device Connected - Retrying in {}s...",
                            seconds
                        ));
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                    continue;
                }
                Err(e) => {
                    for seconds in (1..=5).rev() {
                        device.status.set(format!("❌ Error: {} - Retrying in {}s...", e, seconds));
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                    continue;
                }
            };

            let first_device = &devices[0];
            device.status.set(format!("📱 Found device: {}", first_device.name));
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            device.status.set(format!("🔌 Connecting to {}...", first_device.name));
            let device_name = first_device.name.clone();

            match AdbBackend::new_with_device(&device_name).await {
                Ok(client) => {
                    let (sx, sy) = client.screen_dimensions();
                    device.info.set(Some((
                        client.device_name().to_string(),
                        client.transport_id(),
                        sx,
                        sy,
                    )));
                    device.status.set("✅ Connected".to_string());
                    force_update.with_mut(|v| *v = v.wrapping_add(1));

                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    let shared_client = Arc::new(TokioMutex::new(client));
                    shared_adb_client.set(Some(shared_client.clone()));

                    // Take initial screenshot
                    spawn(async move {
                        screenshot.is_loading.set(true);
                        screenshot.status.set("📸 Taking initial screenshot...".to_string());
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                        let start = std::time::Instant::now();
                        let client_lock = shared_client.lock().await;
                        match client_lock.screen_capture_bytes().await {
                            Ok(bytes) => {
                                let bytes_clone = bytes.clone();
                                let base64_result = tokio::task::spawn_blocking(move || {
                                    base64_encode(&bytes_clone)
                                })
                                .await;
                                match base64_result {
                                    Ok(base64_string) => {
                                        let duration_ms = start.elapsed().as_millis();
                                        let counter_val = screenshot.counter.with_mut(|c| {
                                            *c += 1;
                                            *c
                                        });
                                        screenshot.data.set(Some(base64_string));
                                        screenshot.bytes.set(Some(bytes));
                                        screenshot.status.set(format!(
                                            "✅ Initial screenshot #{} ({}ms)",
                                            counter_val, duration_ms
                                        ));
                                    }
                                    Err(_) => {
                                        screenshot.status
                                            .set("❌ Failed to encode screenshot".to_string());
                                    }
                                }
                                screenshot.is_loading.set(false);
                            }
                            Err(e) => {
                                screenshot.status
                                    .set(format!("❌ Initial screenshot failed: {}", e));
                                screenshot.is_loading.set(false);
                            }
                        }
                    });

                    // === MONITORING PHASE ===
                    // Wait here while device is connected
                    let monitor_shared_client = shared_adb_client.clone();
                    let mut device_status = device.status.clone();
                    
                    // This future will complete when device disconnects
                    let disconnection_detected = async move {
                        let mut check_interval = tokio::time::interval(tokio::time::Duration::from_secs(3));

                        loop {
                            check_interval.tick().await;

                            // If shared_adb_client has been cleared (by FSM reconnection), device is disconnected
                            if monitor_shared_client.read().is_none() {
                                log::debug!("Device monitoring: Client cleared, device disconnected");
                                device_status.set("🔌 Device Disconnected - Searching for device...".to_string());
                                break;
                            }

                            // Just check that client still exists (lightweight check)
                            if let Some(client_arc) = monitor_shared_client.read().clone() {
                                let client_lock = client_arc.lock().await;
                                // Cached operation, doesn't require USB communication
                                let _ = client_lock.screen_dimensions();
                                drop(client_lock);
                            } else {
                                // Client was cleared, exit
                                break;
                            }
                        }

                        log::debug!("Device monitoring task ending, returning to discovery phase");
                    };
                    
                    // Wait for disconnection before going back to discovery phase
                    disconnection_detected.await;
                    
                    // Loop back to discovery phase after device disconnects
                }
                Err(e) => {
                    // Use error helper methods for cleaner code
                    let (get_status, tip_msg, retry_secs): (Box<dyn Fn(&String) -> String>, &str, u32) = 
                        if e.is_resource_busy() {
                            (
                                Box::new(|_e| "⚠️ USB Already in Use - Close other ADB apps - Retrying in {}s...".to_string()),
                                "💡 Close other instances (VS Code, Android Studio, etc.)",
                                10u32,
                            )
                        } else if e.is_permission_denied() {
                            (
                                Box::new(|_e| "⚠️ Permission Denied - Check USB permissions - Retrying in {}s...".to_string()),
                                "💡 Run: sudo chmod 666 /dev/bus/usb/*/0*",
                                5u32,
                            )
                        } else if e.is_device_not_found() {
                            (
                                Box::new(|_e| "⚠️ No Device Found - Reconnect USB cable - Retrying in {}s...".to_string()),
                                "💡 Unplug and replug the USB cable",
                                5u32,
                            )
                        } else {
                            (
                                Box::new(|e: &String| format!("❌ Connection failed: {} - Retrying in {{}}s...", e)),
                                "⏳ Waiting for USB authorization...",
                                5u32,
                            )
                        };

                    for seconds in (1..=retry_secs).rev() {
                        let msg = get_status(&e.to_string());
                        device.status.set(msg.replace("{}", &seconds.to_string()));
                        screenshot.status.set(tip_msg.to_string());
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                }
            }
        }
    });
}
