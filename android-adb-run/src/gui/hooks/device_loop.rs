use crate::adb::{AdbBackend, AdbClient};
use crate::gui::hooks::types::*;
use crate::gui::util::base64_encode;
use dioxus::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

/// Initializes device connection loop that discovers and connects to Android devices
pub fn use_device_loop(
    mut status: Signal<String>,
    mut device_info: DeviceInfoSignal,
    mut is_loading_screenshot: Signal<bool>,
    mut screenshot_status: Signal<String>,
    mut screenshot_data: ScreenshotDataSignal,
    mut screenshot_bytes: ScreenshotBytesSignal,
    mut screenshot_counter: Signal<u64>,
    mut shared_adb_client: SharedAdbClient,
    mut force_update: Signal<u32>,
) {
    use_future(move || async move {
        loop {
            status.set("🔍 Looking for devices...".to_string());
            let devices = match AdbBackend::list_devices().await {
                Ok(devices) if !devices.is_empty() => devices,
                Ok(_) => {
                    for seconds in (1..=5).rev() {
                        status.set(format!(
                            "🔌 No Device Connected - Retrying in {}s...",
                            seconds
                        ));
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                    continue;
                }
                Err(e) => {
                    for seconds in (1..=5).rev() {
                        status.set(format!("❌ Error: {} - Retrying in {}s...", e, seconds));
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                    continue;
                }
            };

            let first_device = &devices[0];
            status.set(format!("📱 Found device: {}", first_device.name));
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            status.set(format!("🔌 Connecting to {}...", first_device.name));
            let device_name = first_device.name.clone();

            match AdbBackend::new_with_device(&device_name).await {
                Ok(client) => {
                    let (sx, sy) = client.screen_dimensions();
                    device_info.set(Some((
                        client.device_name().to_string(),
                        client.transport_id(),
                        sx,
                        sy,
                    )));
                    status.set("✅ Connected".to_string());
                    force_update.with_mut(|v| *v = v.wrapping_add(1));

                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    let shared_client = Arc::new(TokioMutex::new(client));
                    shared_adb_client.set(Some(shared_client.clone()));

                    spawn(async move {
                        is_loading_screenshot.set(true);
                        screenshot_status.set("📸 Taking initial screenshot...".to_string());
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
                                        let counter_val = screenshot_counter.with_mut(|c| {
                                            *c += 1;
                                            *c
                                        });
                                        screenshot_data.set(Some(base64_string));
                                        screenshot_bytes.set(Some(bytes));
                                        screenshot_status.set(format!(
                                            "✅ Initial screenshot #{} ({}ms)",
                                            counter_val, duration_ms
                                        ));
                                    }
                                    Err(_) => {
                                        screenshot_status
                                            .set("❌ Failed to encode screenshot".to_string());
                                    }
                                }
                                is_loading_screenshot.set(false);
                            }
                            Err(e) => {
                                screenshot_status
                                    .set(format!("❌ Initial screenshot failed: {}", e));
                                is_loading_screenshot.set(false);
                            }
                        }
                    });
                    break;
                }
                Err(e) => {
                    for seconds in (1..=5).rev() {
                        status.set(format!(
                            "❌ Connection failed: {} - Retrying in {}s...",
                            e, seconds
                        ));
                        screenshot_status.set("⏳ Waiting for USB authorization...".to_string());
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                }
            }
        }
    });
}
