use super::template_matching_pipeline::{decode_screenshot_to_rgb, start_template_matching_phase};
use crate::adb::AdbClient;
use crate::gui::hooks::types::ScreenshotSignals;
use crate::gui::util::base64_encode;
use dioxus::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

pub(super) fn spawn_initial_screenshot_task(
    mut screenshot: ScreenshotSignals,
    shared_client: Arc<TokioMutex<crate::adb::AdbBackend>>,
) {
    dioxus::prelude::spawn(async move {
        screenshot.is_loading.set(true);
        screenshot
            .status
            .set("📸 Taking initial screenshot...".to_string());
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let start = std::time::Instant::now();
        let client_lock = shared_client.lock().await;
        match client_lock.screen_capture_bytes().await {
            Ok(bytes) => {
                let bytes_clone = bytes.clone();
                let (base64_string, rgb_image) = tokio::task::spawn_blocking(move || {
                    let b64 = base64_encode(&bytes_clone);
                    let rgb = decode_screenshot_to_rgb(&bytes_clone).ok();
                    (b64, rgb)
                })
                .await
                .unwrap_or_else(|_| ("Error: encoding failed".to_string(), None));

                let duration_ms = start.elapsed().as_millis();
                let counter_val = screenshot.counter.with_mut(|c| {
                    *c += 1;
                    *c
                });

                screenshot.data.set(Some(base64_string));
                screenshot.bytes.set(Some(bytes.clone()));
                screenshot.status.set(format!(
                    "✅ Screenshot #{} displayed ({}ms) - Matching...",
                    counter_val, duration_ms
                ));
                screenshot.is_loading.set(false);

                let status_signal = screenshot.status;
                let status_history_signal = screenshot.status_history;
                start_template_matching_phase(
                    bytes.clone(),
                    rgb_image,
                    counter_val as u32,
                    status_signal,
                    status_history_signal,
                );
            }
            Err(e) => {
                screenshot
                    .status
                    .set(format!("❌ Initial screenshot failed: {}", e));
                screenshot.is_loading.set(false);
            }
        }
    });
}
