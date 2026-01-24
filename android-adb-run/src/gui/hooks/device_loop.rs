use crate::adb::{AdbBackend, AdbClient};
use crate::gui::hooks::types::*;
use crate::gui::util::base64_encode;
use crate::template_matching::{PatchInfo, TemplateMatcher};
use dioxus::prelude::*;
use image::{ImageReader, RgbImage};
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

/// Type alias for error handling configuration
type ErrorConfig = (Box<dyn Fn(&String) -> String>, &'static str, u32);

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
                        device
                            .status
                            .set(format!("❌ Error: {} - Retrying in {}s...", e, seconds));
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                    continue;
                }
            };

            let first_device = &devices[0];
            device
                .status
                .set(format!("📱 Found device: {}", first_device.name));
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            device
                .status
                .set(format!("🔌 Connecting to {}...", first_device.name));
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
                        screenshot
                            .status
                            .set("📸 Taking initial screenshot...".to_string());
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                        let start = std::time::Instant::now();
                        let client_lock = shared_client.lock().await;
                        match client_lock.screen_capture_bytes().await {
                            Ok(bytes) => {
                                // Phase 1: Decode image and encode to base64 (blocking operations)
                                let bytes_clone = bytes.clone();
                                let (base64_string, rgb_image) =
                                    tokio::task::spawn_blocking(move || {
                                        let b64 = base64_encode(&bytes_clone);
                                        let rgb = decode_screenshot_to_rgb(&bytes_clone).ok();
                                        (b64, rgb)
                                    })
                                    .await
                                    .unwrap_or_else(|_| {
                                        ("Error: encoding failed".to_string(), None)
                                    });

                                let duration_ms = start.elapsed().as_millis();
                                let counter_val = screenshot.counter.with_mut(|c| {
                                    *c += 1;
                                    *c
                                });

                                // Phase 2: Display image immediately
                                screenshot.data.set(Some(base64_string));
                                screenshot.bytes.set(Some(bytes.clone()));
                                screenshot.status.set(format!(
                                    "✅ Screenshot #{} displayed ({}ms) - Matching...",
                                    counter_val, duration_ms
                                ));
                                screenshot.is_loading.set(false);

                                // Phase 3: Run template matching in dedicated thread (after image is displayed)
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

                    // === MONITORING PHASE ===
                    // Wait here while device is connected
                    let monitor_shared_client = shared_adb_client;
                    let mut device_status = device.status;

                    // This future will complete when device disconnects
                    let disconnection_detected = async move {
                        let mut check_interval =
                            tokio::time::interval(tokio::time::Duration::from_secs(3));

                        loop {
                            check_interval.tick().await;

                            // If shared_adb_client has been cleared (by FSM reconnection), device is disconnected
                            if monitor_shared_client.read().is_none() {
                                log::debug!(
                                    "Device monitoring: Client cleared, device disconnected"
                                );
                                device_status.set(
                                    "🔌 Device Disconnected - Searching for device...".to_string(),
                                );
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
                    let (get_status, tip_msg, retry_secs): ErrorConfig = if e.is_resource_busy() {
                        (
                            Box::new(|_e| {
                                "⚠️ USB Already in Use - Close other ADB apps - Retrying in {}s..."
                                    .to_string()
                            }),
                            "💡 Close other instances (VS Code, Android Studio, etc.)",
                            10u32,
                        )
                    } else if e.is_permission_denied() {
                        (
                            Box::new(|_e| {
                                "⚠️ Permission Denied - Check USB permissions - Retrying in {}s..."
                                    .to_string()
                            }),
                            "💡 Run: sudo chmod 666 /dev/bus/usb/*/0*",
                            5u32,
                        )
                    } else if e.is_device_not_found() {
                        (
                            Box::new(|_e| {
                                "⚠️ No Device Found - Reconnect USB cable - Retrying in {}s..."
                                    .to_string()
                            }),
                            "💡 Unplug and replug the USB cable",
                            5u32,
                        )
                    } else {
                        (
                            Box::new(|e: &String| {
                                format!("❌ Connection failed: {} - Retrying in {{}}s...", e)
                            }),
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

/// Start template matching phase for a screenshot
/// Helper function to add messages to history with automatic filtering
///
/// When a result message is added, removes any previous progress messages for the same patch.
/// On the first result, also clears setup messages (Loading, Scanning, etc.)
fn add_history_message(
    history_signal: &mut Signal<Vec<(String, bool)>>,
    message: String,
    is_result: bool,
    patch_name: Option<&str>,
    is_first_result: bool,
) {
    // Clone current history so we can create a new vector for this update
    let mut history = {
        let history_ref = history_signal.read();
        history_ref.clone()
    };

    if is_result {
        if is_first_result {
            // First result: keep only result messages
            history.retain(|(_, is_res)| *is_res);
        } else if let Some(patch) = patch_name {
            // Not first result: remove progress messages for this specific patch
            history.retain(|(msg, is_res)| {
                // Keep all result messages
                *is_res
                    || (
                        // Keep progress messages that don't belong to this patch
                        !msg.contains("⏳ Still matching") && !msg.contains("🔎 Checking")
                    )
                    || !msg.contains(patch)
            });
        }
    }

    // Add the new message
    history.push((message, is_result));

    // Keep only last 15 messages
    if history.len() > 15 {
        let excess = history.len() - 15;
        history.drain(0..excess);
    }

    // Replace the signal contents so the UI rerenders
    history_signal.set(history);
}

///
/// Takes screenshot data and optional RGB image, spawns matching in a thread pool,
/// and updates the status_signal with progress messages in real-time
pub fn start_template_matching_phase(
    bytes: Vec<u8>,
    rgb_image: Option<RgbImage>,
    screenshot_counter: u32,
    mut status_signal: Signal<String>,
    mut status_history_signal: Signal<Vec<(String, bool)>>,
) {
    spawn(async move {
        log::info!("🚀 PHASE 3 STARTING - Template matching");

        // Don't clear history to avoid race conditions with concurrent screenshots.
        // History will be naturally pruned when it exceeds 15 messages in add_history_message().
        // This prevents the UI from appearing stuck when multiple screenshots are taken in quick succession.

        // Create a channel for progress updates from the blocking task
        // Use larger buffer (500) to avoid blocking during high-frequency progress updates
        // Channel sends (message, is_result) tuples for robust filtering
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, bool)>(500);

        // Show that we're starting patch management
        let init_msg = format!("[#{}] 🔍 Loading patches...", screenshot_counter);
        log::info!("📝 Setting initial status: {}", init_msg);
        status_signal.set(init_msg.clone());
        log::info!("📊 Before adding init message - history about to get new message");
        add_history_message(&mut status_history_signal, init_msg, false, None, false);
        log::info!("📊 After adding init message");

        // Run matching in a separate thread pool to avoid blocking UI
        log::info!("🧵 Spawning blocking task for match_patches_blocking_with_progress");
        let mut result_handle = tokio::task::spawn_blocking(move || {
            log::info!("🔧 Inside spawn_blocking - calling match_patches_blocking_with_progress");
            match_patches_blocking_with_progress(&bytes, rgb_image, screenshot_counter, tx)
        });

        log::info!("⏳ Starting message receive loop");
        // Process progress messages as they arrive, waiting for the result
        let mut result = None;
        let mut first_result_received = false;
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    if let Some((progress_msg, is_result)) = msg {
                        log::info!("📨 Received progress message: {} (is_result: {})", progress_msg, is_result);
                        status_signal.set(progress_msg.clone());

                        // Extract patch name if it's a result message
                        let patch_name = if is_result {
                            // Extract patch name from message like "[#1] ✓ Matched patch_name in 1s (90%)"
                            // Look for text between the emoji and " in"
                            progress_msg.split(']').nth(1)
                                .and_then(|s| s.split_whitespace().nth(2))
                        } else {
                            None
                        };

                        add_history_message(
                            &mut status_history_signal,
                            progress_msg.clone(),
                            is_result,
                            patch_name,
                            is_result && !first_result_received,
                        );

                        if is_result {
                            first_result_received = true;
                        }
                    } else {
                        log::info!("📭 Channel closed");
                        // Channel closed, break to wait for result
                        break;
                    }
                }
                res = &mut result_handle => {
                    log::info!("✅ Blocking task completed");
                    result = Some(res);
                    // Continue to drain remaining messages before exiting
                    while let Ok((progress_msg, is_result)) = rx.try_recv() {
                        log::info!("🗑️ Draining queued message: {} (is_result: {})", progress_msg, is_result);
                        status_signal.set(progress_msg.clone());

                        let patch_name = if is_result {
                            progress_msg.split(']').nth(1)
                                .and_then(|s| s.split_whitespace().nth(2))
                        } else {
                            None
                        };

                        add_history_message(
                            &mut status_history_signal,
                            progress_msg.clone(),
                            is_result,
                            patch_name,
                            is_result && !first_result_received,
                        );

                        if is_result {
                            first_result_received = true;
                        }
                    }
                    break;
                }
            }
        }

        let result = result.unwrap_or(Ok(None));
        log::info!("✅ PHASE 3 Complete - Final result: {:?}", result);
    });
}

/// Start template matching phase for a screenshot
///
/// Sends progress messages through the channel as matching proceeds
/// Returns the best matching patch name, if any match is found above threshold
fn match_patches_blocking_with_progress(
    screenshot_bytes: &[u8],
    image_rgb: Option<RgbImage>,
    screenshot_counter: u32,
    tx: tokio::sync::mpsc::Sender<(String, bool)>,
) -> Option<String> {
    // Use pre-decoded image or decode if not provided
    let image_rgb = match image_rgb {
        Some(img) => img,
        None => match decode_screenshot_to_rgb(screenshot_bytes) {
            Ok(img) => img,
            Err(_) => return None,
        },
    };

    // Load patches from assets directory (blocking I/O)
    let patch_dir = std::path::Path::new("assets/test_images");

    if !patch_dir.exists() {
        log::debug!("Patch directory not found: {:?}", patch_dir);
        let _ = tx.blocking_send((format!("[#{}] ⚠️ Patch directory not found", screenshot_counter), false));
        return None;
    }

    let mut matcher = TemplateMatcher::new();
    let mut patch_count = 0;

    log::debug!("🔍 Starting patch matching");
    let send_result = tx.blocking_send((format!("[#{}] 🔍 Scanning patches...", screenshot_counter), false));
    log::debug!("📤 Sent 'Scanning patches' message: {:?}", send_result);

    match std::fs::read_dir(patch_dir) {
        Ok(entries) => {
            let entries_vec: Vec<_> = entries.flatten().collect();
            log::debug!("📂 Found {} files in patch directory", entries_vec.len());

            for (idx, entry) in entries_vec.iter().enumerate() {
                let path = entry.path();
                let filename = match path.file_name() {
                    Some(name) => match name.to_str() {
                        Some(s) => s.to_string(),
                        None => continue,
                    },
                    None => continue,
                };

                // Look for patch-*.png files
                if !filename.starts_with("patch-") || !filename.ends_with(".png") {
                    continue;
                }

                // Parse patch filename to extract label and coordinates
                if let Some((label, x, y, width, height)) = parse_patch_filename(&filename) {
                    match std::fs::read(&path) {
                        Ok(pixel_data) => {
                            // Decode the patch image to get RGB pixels
                            match decode_screenshot_to_rgb(&pixel_data) {
                                Ok(img) => {
                                    let pixels = img.into_raw();
                                    let patch = PatchInfo::new(label, x, y, width, height, pixels);
                                    matcher.add_patch(patch);
                                    patch_count += 1;
                                    log::debug!(
                                        "✓ Loaded patch {} ({}/{})",
                                        filename,
                                        idx + 1,
                                        entries_vec.len()
                                    );
                                }
                                Err(_) => continue,
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }

            // Send consolidated message after all patches are loaded
            if patch_count > 0 {
                let msg = format!("[#{}] 📦 Loaded {} patches to match...", screenshot_counter, patch_count);
                let _ = tx.blocking_send((msg, false));
            }
        }
        Err(_) => {
            log::error!("Failed to read patch directory");
            let _ = tx.blocking_send((format!("[#{}] ⚠️ Failed to load patches", screenshot_counter), false));
            return None;
        }
    }

    if patch_count == 0 {
        log::debug!("⚠️ No patches loaded");
        let _ = tx.blocking_send((format!("[#{}] ⚠️ No patches found", screenshot_counter), false));
        return None;
    }

    log::debug!(
        "📊 Loaded {} patches, starting correlation matching",
        patch_count
    );
    if let Err(e) = tx.blocking_send((format!("[#{}] 🔎 Matching {} patches...", screenshot_counter, patch_count), false)) {
        log::error!("❌ Failed to send 'Matching patches' message: {}", e);
    }

    // Find the best match across all patches
    let threshold = 0.85; // 85% correlation threshold
    let mut best_match: Option<(String, f32)> = None;

    log::info!(
        "🔄 Starting matching loop with {} patches",
        matcher.patches().len()
    );

    for (idx, patch) in matcher.patches().iter().enumerate() {
        let patch_name = patch.display_name();
        log::info!(
            "🔄 Matching patch {} of {}: {}",
            idx + 1,
            matcher.patches().len(),
            patch_name
        );

        // Send message showing which patch we're checking
        let progress_pct = ((idx + 1) as f32 / matcher.patches().len() as f32 * 100.0) as u32;
        let msg = format!("[#{}] 🔎 Checking {}... ({}%)", screenshot_counter, patch_name, progress_pct);
        if let Err(e) = tx.blocking_send((msg.clone(), false)) {
            log::error!("❌ Failed to send patch check message: {}", e);
        } else {
            log::info!("📤 Sent: {}", msg);
        }

        let start = std::time::Instant::now();
        let total_patches = matcher.patches().len();
        let patches_completed = idx;

        // Use a flag to signal the progress thread to stop
        let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        // Spawn a thread to send periodic progress messages while matching
        let tx_clone = tx.clone();
        let patch_name_clone = patch_name.clone();
        let screenshot_counter_clone = screenshot_counter;
        let progress_handle = std::thread::spawn(move || {
            let mut counter = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(30));

                // Check if we should stop
                if stop_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                    log::debug!("🛑 Progress thread stopping");
                    break;
                }

                counter += 1;
                let elapsed_secs = counter * 30;

                // Estimate remaining time based on patches completed
                let estimated_secs_per_patch = if patches_completed > 0 {
                    // This will be updated with actual elapsed time later
                    elapsed_secs / patches_completed as u32
                } else {
                    60 // Default estimate
                };

                let remaining_patches = (total_patches - patches_completed - 1) as u32;
                let estimated_remaining = estimated_secs_per_patch * remaining_patches;

                let msg = if estimated_remaining > 0 {
                    format!(
                        "[#{}] ⏳ Still matching {}... ({} sec, ~{} min remaining)",
                        screenshot_counter_clone,
                        patch_name_clone,
                        elapsed_secs,
                        estimated_remaining / 60
                    )
                } else {
                    format!(
                        "[#{}] ⏳ Still matching {}... ({} sec)",
                        screenshot_counter_clone, patch_name_clone, elapsed_secs
                    )
                };

                if tx_clone.blocking_send((msg, false)).is_err() {
                    break; // Channel closed, matching is done
                }
            }
        });

        let matches = matcher.find_matches(&image_rgb, idx, threshold, 1, 50);
        let elapsed = start.elapsed();

        // Signal the progress thread to stop immediately
        stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);

        // Send completion message with match status and accuracy
        let completion_msg = if let Some(m) = matches.first() {
            let accuracy_pct = (m.correlation * 100.0) as u32;
            format!(
                "[#{}] ✓ Matched {} in {:.0}s ({}%)",
                screenshot_counter,
                patch_name,
                elapsed.as_secs_f32(),
                accuracy_pct
            )
        } else {
            format!(
                "[#{}] ✗ No match for {} ({:.0}s)",
                screenshot_counter,
                patch_name,
                elapsed.as_secs_f32()
            )
        };
        let _ = tx.blocking_send((completion_msg, true)); // true = this is a result message
        log::info!(
            "✓ find_matches completed for {} in {:.2}s, found {} matches",
            patch_name,
            elapsed.as_secs_f32(),
            matches.len()
        );

        // Wait for progress thread to exit
        let _ = progress_handle.join();

        if let Some(m) = matches.first() {
            // Found a match
            let patch_name = patch.display_name();
            log::debug!(
                "🎯 Found match: {} (correlation: {:.1}%)",
                patch_name,
                m.correlation * 100.0
            );
            if best_match.is_none() || m.correlation > best_match.as_ref().unwrap().1 {
                best_match = Some((patch_name, m.correlation));
            }
        }
    }

    match &best_match {
        Some((name, correlation)) => {
            log::debug!("✅ Best match: {} ({:.1}%)", name, correlation * 100.0);
            let accuracy_pct = (correlation * 100.0) as u32;
            if let Err(e) = tx.blocking_send((format!("[#{}] 🎯 BEST MATCH: {} ({}%)", screenshot_counter, name, accuracy_pct), true))
            {
                log::error!("❌ Failed to send final match message: {}", e);
            }
        }
        None => {
            log::debug!("❌ No matches found");
            if let Err(e) = tx.blocking_send((format!("[#{}] ⚠️ No matches found in any patch", screenshot_counter), true)) {
                log::error!("❌ Failed to send no-match message: {}", e);
            }
        }
    }

    best_match.map(|(name, _)| name)
}

/// Helper function to match patches synchronously (runs in thread pool)
///
/// This is kept as a fallback for when progress reporting is not needed.
/// Returns the best matching patch name, if any match is found above threshold
#[allow(dead_code)]
fn match_patches_blocking(screenshot_bytes: &[u8], image_rgb: Option<RgbImage>) -> Option<String> {
    // Use pre-decoded image or decode if not provided
    let image_rgb = match image_rgb {
        Some(img) => img,
        None => match decode_screenshot_to_rgb(screenshot_bytes) {
            Ok(img) => img,
            Err(_) => return None,
        },
    };

    // Load patches from assets directory (blocking I/O)
    let patch_dir = std::path::Path::new("assets/test_images");

    if !patch_dir.exists() {
        log::debug!("Patch directory not found: {:?}", patch_dir);
        return None;
    }

    let mut matcher = TemplateMatcher::new();
    let mut patch_count = 0;

    log::debug!("🔍 Starting patch matching");

    match std::fs::read_dir(patch_dir) {
        Ok(entries) => {
            let entries_vec: Vec<_> = entries.flatten().collect();
            log::debug!("📂 Found {} files in patch directory", entries_vec.len());

            for (idx, entry) in entries_vec.iter().enumerate() {
                let path = entry.path();
                let filename = match path.file_name() {
                    Some(name) => match name.to_str() {
                        Some(s) => s.to_string(),
                        None => continue,
                    },
                    None => continue,
                };

                // Look for patch-*.png files
                if !filename.starts_with("patch-") || !filename.ends_with(".png") {
                    continue;
                }

                // Parse patch filename to extract label and coordinates
                if let Some((label, x, y, width, height)) = parse_patch_filename(&filename) {
                    match std::fs::read(&path) {
                        Ok(pixel_data) => {
                            // Decode the patch image to get RGB pixels
                            match decode_screenshot_to_rgb(&pixel_data) {
                                Ok(img) => {
                                    let pixels = img.into_raw();
                                    let patch = PatchInfo::new(label, x, y, width, height, pixels);
                                    matcher.add_patch(patch);
                                    patch_count += 1;
                                    log::debug!(
                                        "✓ Loaded patch {} ({}/{})",
                                        filename,
                                        idx + 1,
                                        entries_vec.len()
                                    );
                                }
                                Err(_) => continue,
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }
        }
        Err(_) => {
            log::error!("Failed to read patch directory");
            return None;
        }
    }

    if patch_count == 0 {
        log::debug!("⚠️ No patches loaded");
        return None;
    }

    log::debug!(
        "📊 Loaded {} patches, starting correlation matching",
        patch_count
    );

    // Find the best match across all patches
    let threshold = 0.85; // 85% correlation threshold
    let mut best_match: Option<(String, f32)> = None;

    for (idx, patch) in matcher.patches().iter().enumerate() {
        let matches = matcher.find_matches(&image_rgb, idx, threshold, 1, 50);

        let progress_pct = ((idx + 1) as f32 / matcher.patches().len() as f32 * 100.0) as u32;
        log::debug!("⏳ Matching progress: {}%", progress_pct);

        if let Some(m) = matches.first() {
            // Found a match
            let patch_name = patch.display_name();
            log::debug!(
                "🎯 Found match: {} (correlation: {:.1}%)",
                patch_name,
                m.correlation * 100.0
            );
            if best_match.is_none() || m.correlation > best_match.as_ref().unwrap().1 {
                best_match = Some((patch_name, m.correlation));
            }
        }
    }

    match &best_match {
        Some((name, correlation)) => {
            log::debug!("✅ Best match: {} ({:.1}%)", name, correlation * 100.0);
        }
        None => {
            log::debug!("❌ No matches found");
        }
    }

    best_match.map(|(name, _)| name)
}

/// Helper function to match patches in screenshot bytes (async wrapper for compatibility)
///
/// Returns the best matching patch name, if any match is found above threshold
#[allow(dead_code)]
async fn match_patches(screenshot_bytes: &[u8]) -> Option<String> {
    // Try to decode screenshot to RGB image
    let image_rgb = match decode_screenshot_to_rgb(screenshot_bytes) {
        Ok(img) => img,
        Err(_) => return None,
    };

    match_patches_with_rgb(screenshot_bytes, Some(image_rgb)).await
}

/// Helper function to match patches using a pre-decoded RGB image (faster)
///
/// Returns the best matching patch name, if any match is found above threshold
#[allow(dead_code)]
async fn match_patches_with_rgb(
    screenshot_bytes: &[u8],
    image_rgb: Option<RgbImage>,
) -> Option<String> {
    // Use pre-decoded image or decode if not provided
    let image_rgb = match image_rgb {
        Some(img) => img,
        None => match decode_screenshot_to_rgb(screenshot_bytes) {
            Ok(img) => img,
            Err(_) => return None,
        },
    };

    // Load patches from assets directory
    let mut matcher = TemplateMatcher::new();
    if !load_patches(&mut matcher).await {
        // No patches available
        return None;
    }

    // Find the best match across all patches
    let threshold = 0.85; // 85% correlation threshold
    let mut best_match: Option<(String, f32)> = None;

    for (idx, patch) in matcher.patches().iter().enumerate() {
        let matches = matcher.find_matches(&image_rgb, idx, threshold, 1, 50);

        if let Some(m) = matches.first() {
            // Found a match
            let patch_name = patch.display_name();
            if best_match.is_none() || m.correlation > best_match.as_ref().unwrap().1 {
                best_match = Some((patch_name, m.correlation));
            }
        }
    }

    best_match.map(|(name, _)| name)
}

/// Decode screenshot bytes to RGB image
pub fn decode_screenshot_to_rgb(bytes: &[u8]) -> Result<RgbImage, String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess format: {}", e))?;

    let image = reader
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let rgb = image.to_rgb8();
    Ok(rgb)
}

/// Load all available patches from assets directory
async fn load_patches(matcher: &mut TemplateMatcher) -> bool {
    use std::path::Path;
    use tokio::fs;

    let patch_dir = Path::new("assets/test_images");

    if !patch_dir.exists() {
        log::debug!("Patch directory not found: {:?}", patch_dir);
        return false;
    }

    let mut entries = match fs::read_dir(patch_dir).await {
        Ok(entries) => entries,
        Err(e) => {
            log::error!("Failed to read patch directory: {}", e);
            return false;
        }
    };

    let mut patch_count = 0;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let filename = match path.file_name() {
            Some(name) => match name.to_str() {
                Some(s) => s.to_string(),
                None => continue,
            },
            None => continue,
        };

        // Look for patch-*.png files
        if !filename.starts_with("patch-") || !filename.ends_with(".png") {
            continue;
        }

        // Parse patch filename to extract label and coordinates
        if let Some((label, x, y, width, height)) = parse_patch_filename(&filename) {
            match fs::read(&path).await {
                Ok(pixel_data) => {
                    // Try to decode the patch image to get RGB pixels
                    match decode_screenshot_to_rgb(&pixel_data) {
                        Ok(img) => {
                            let pixels = img.into_raw();
                            let patch_info = PatchInfo::new(label, x, y, width, height, pixels);
                            matcher.add_patch(patch_info);
                            patch_count += 1;
                        }
                        Err(e) => {
                            log::warn!("Failed to decode patch {}: {}", filename, e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to read patch file {}: {}", filename, e);
                }
            }
        }
    }

    log::debug!("Loaded {} patches for matching", patch_count);
    patch_count > 0
}

/// Parse patch filename to extract label and coordinates
/// Format: patch-[label-][x,y,width,height].png
fn parse_patch_filename(filename: &str) -> Option<(Option<String>, u32, u32, u32, u32)> {
    // Remove .png extension
    let name = filename.strip_suffix(".png")?;

    // Remove "patch-" prefix
    if !name.starts_with("patch-") {
        return None;
    }
    let name = &name[6..];

    // Find the last '[' to identify coordinates
    let bracket_pos = name.rfind('[')?;
    let label_part = &name[..bracket_pos];
    let coords_part = &name[bracket_pos..];

    // Parse coordinates [x,y,width,height]
    if !coords_part.starts_with('[') || !coords_part.ends_with(']') {
        return None;
    }

    let coords_str = &coords_part[1..coords_part.len() - 1];
    let parts: Vec<&str> = coords_str.split(',').collect();
    if parts.len() != 4 {
        return None;
    }

    let x = parts[0].trim().parse::<u32>().ok()?;
    let y = parts[1].trim().parse::<u32>().ok()?;
    let width = parts[2].trim().parse::<u32>().ok()?;
    let height = parts[3].trim().parse::<u32>().ok()?;

    // Parse label (may be empty if no label)
    let label = if label_part.is_empty() {
        None
    } else {
        // Remove trailing dash if present
        let label_str = if let Some(stripped) = label_part.strip_suffix('-') {
            stripped
        } else {
            label_part
        };
        Some(label_str.to_string())
    };

    Some((label, x, y, width, height))
}
