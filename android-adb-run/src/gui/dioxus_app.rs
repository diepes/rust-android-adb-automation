use crate::adb::{AdbBackend, AdbClient};
use crate::game_automation::types::TimedEvent;
use crate::game_automation::{
    AutomationCommand, AutomationEvent, GameAutomation, GameState, create_automation_channels,
};
use crate::gui::components::{
    actions::Actions,
    device_info::DeviceInfo,
    screenshot_panel::{TapMarker, screenshot_panel},
};
use crate::gui::util::base64_encode;
use dioxus::prelude::*;
use std::sync::OnceLock;
use tokio::sync::mpsc;

const APP_VERSION: &str = env!("APP_VERSION_DISPLAY");
const BUILD_YEAR: &str = env!("APP_BUILD_YEAR");

// Global state to store the debug mode choice
static DEBUG_MODE: OnceLock<bool> = OnceLock::new();

pub fn is_debug_mode() -> bool {
    *DEBUG_MODE.get().unwrap_or(&false)
}

pub fn run_gui(debug_mode: bool) {
    DEBUG_MODE
        .set(debug_mode)
        .expect("DEBUG_MODE should only be set once");

    use dioxus::desktop::{Config, WindowBuilder};
    let enable_borderless = false; // Use custom borderless window for better cross-platform control
    let window_title = format!(
        "Android ADB Automation v{} (Build {})",
        APP_VERSION, BUILD_YEAR
    );
    let config = Config::new()
        .with_window(
            WindowBuilder::new()
                .with_title(window_title)
                .with_decorations(!enable_borderless) // false => no native title/menu (custom controls)
                .with_resizable(true)
                .with_inner_size(dioxus::desktop::LogicalSize::new(1000, 700)),
        )
        .with_menu(None); // Disable the menu bar (removes [Window] and [Edit])

    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(App);
}

#[component]
fn App() -> Element {
    let mut status = use_signal(|| "Initializing...".to_string());
    let mut device_info = use_signal(|| None::<(String, Option<u32>, u32, u32)>);
    let mut screenshot_data = use_signal(|| None::<String>);
    let mut screenshot_bytes = use_signal(|| None::<Vec<u8>>);
    let mut screenshot_status = use_signal(|| "".to_string());
    let mut screenshot_counter = use_signal(|| 0u64); // GUI-level screenshot counter
    let mouse_coords = use_signal(|| None::<(i32, i32)>);
    let device_coords = use_signal(|| None::<(u32, u32)>);
    let auto_update_on_touch = use_signal(|| true);
    let select_box = use_signal(|| false);
    let mut is_loading_screenshot = use_signal(|| false);

    // Game automation state
    let automation_state = use_signal(|| GameState::Idle);
    let automation_command_tx = use_signal(|| None::<mpsc::Sender<AutomationCommand>>);
    let timed_tap_countdown = use_signal(|| None::<(String, u64)>); // (id, seconds_remaining)
    let timed_events_list = use_signal(Vec::<TimedEvent>::new); // All timed events
    let is_paused_by_touch = use_signal(|| false); // New signal for touch-based pause state
    let touch_timeout_remaining = use_signal(|| None::<u64>); // Remaining seconds until touch timeout expires

    let selection_start = use_signal(|| None::<dioxus::html::geometry::ElementPoint>);
    let selection_end = use_signal(|| None::<dioxus::html::geometry::ElementPoint>);

    // Swipe gesture state
    let is_swiping = use_signal(|| false);
    let swipe_start = use_signal(|| None::<(u32, u32)>);
    let swipe_end = use_signal(|| None::<(u32, u32)>);

    let tap_markers = use_signal(Vec::<TapMarker>::new);

    let runtime_days = use_signal(|| 0.0f64);
    let hover_tap_preview = use_signal(|| None::<(u32, u32)>);

    use_effect(move || {
        let runtime_days_signal = runtime_days;
        spawn(async move {
            let mut runtime_days_signal = runtime_days_signal;
            let start = std::time::Instant::now();
            loop {
                let elapsed_days = start.elapsed().as_secs_f64() / 86_400.0;
                runtime_days_signal.set(elapsed_days);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });
    });

    // Helper function to calculate device coordinates from mouse coordinates (correcting for image border)
    fn calculate_device_coords(
        element_rect: dioxus::html::geometry::ElementPoint,
        screen_x: u32,
        screen_y: u32,
    ) -> (u32, u32) {
        let max_display_width = 400.0;
        let max_display_height = 600.0;
        let border_px = 8.0; // image border thickness

        // Derive displayed image (content) size from aspect ratio constraints
        let image_aspect = screen_x as f32 / screen_y as f32;
        let container_aspect = max_display_width / max_display_height;
        let (outer_w, outer_h) = if image_aspect > container_aspect {
            (max_display_width, max_display_width / image_aspect)
        } else {
            (max_display_height * image_aspect, max_display_height)
        };
        // Remove border from both sides
        let displayed_w = (outer_w - border_px * 2.0).max(1.0);
        let displayed_h = (outer_h - border_px * 2.0).max(1.0);

        // Adjust raw coordinates by border offset
        let raw_x = element_rect.x as f32 - border_px;
        let raw_y = element_rect.y as f32 - border_px;

        // Clamp within displayed content
        let clamped_x_in_display = raw_x.max(0.0).min(displayed_w - 1.0);
        let clamped_y_in_display = raw_y.max(0.0).min(displayed_h - 1.0);

        // Scale to device coordinates
        let scale_x = screen_x as f32 / displayed_w;
        let scale_y = screen_y as f32 / displayed_h;
        let device_x = (clamped_x_in_display * scale_x) as u32;
        let device_y = (clamped_y_in_display * scale_y) as u32;

        (device_x.min(screen_x - 1), device_y.min(screen_y - 1))
    }

    // Initialize ADB connection on first render - fully async with progressive UI updates
    use_effect(move || {
        spawn(async move {
            // Retry loop for device connection
            loop {
                // Step 1: Look for devices (fast operation)
                status.set("🔍 Looking for devices...".to_string());

                let devices = match AdbBackend::list_devices().await {
                    Ok(devices) if !devices.is_empty() => devices,
                    Ok(_) => {
                        // Countdown timer for retry (10 seconds)
                        for seconds_remaining in (1..=10).rev() {
                            status.set(format!(
                                "🔌 No Device Connected - Retrying in {}s...",
                                seconds_remaining
                            ));
                            screenshot_status.set(format!(
                                "⏳ Connect your device via USB... ({}/10)",
                                11 - seconds_remaining
                            ));
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                        continue; // Retry
                    }
                    Err(e) => {
                        // Countdown timer for retry (10 seconds)
                        for seconds_remaining in (1..=10).rev() {
                            status.set(format!(
                                "❌ Error: {} - Retrying in {}s...",
                                e, seconds_remaining
                            ));
                            screenshot_status.set(format!(
                                "⏳ Connect your device via USB... ({}/10)",
                                11 - seconds_remaining
                            ));
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                        continue; // Retry
                    }
                };

                let first_device = &devices[0];

                // Step 2: Update GUI immediately with found device info
                status.set(format!("📱 Found device: {}", first_device.name));

                // Small delay to let UI update
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                // Step 3: Start connection process
                status.set(format!("🔌 Connecting to {}...", first_device.name));

                // Step 4: Connect to device in background, update GUI when ready
                spawn({
                    let device_name = first_device.name.clone();
                    async move {
                        match AdbBackend::new_with_device(&device_name).await {
                            Ok(client) => {
                                // Step 4: Connection successful, update device info immediately
                                let (sx, sy) = client.screen_dimensions();
                                device_info.set(Some((
                                    client.device_name().to_string(),
                                    client.transport_id(),
                                    sx,
                                    sy,
                                )));
                                status.set("✅ Connected".to_string());

                                // Step 5: Take initial screenshot in background, don't block UI
                                spawn(async move {
                                    is_loading_screenshot.set(true);
                                    screenshot_status
                                        .set("📸 Taking initial screenshot...".to_string());
                                    let start = std::time::Instant::now();

                                    match client.screen_capture_bytes().await {
                                        Ok(bytes) => {
                                            // Move heavy base64 encoding to background thread
                                            let bytes_clone = bytes.clone();
                                            let base64_result =
                                                tokio::task::spawn_blocking(move || {
                                                    base64_encode(&bytes_clone)
                                                })
                                                .await;

                                            match base64_result {
                                                Ok(base64_string) => {
                                                    let duration_ms = start.elapsed().as_millis();
                                                    let counter_val =
                                                        screenshot_counter.with_mut(|c| {
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
                                                    screenshot_status.set(
                                                        "❌ Failed to encode screenshot"
                                                            .to_string(),
                                                    );
                                                }
                                            }
                                            is_loading_screenshot.set(false);
                                        }
                                        Err(e) => {
                                            screenshot_status.set(format!(
                                                "❌ Initial screenshot failed: {}",
                                                e
                                            ));
                                            is_loading_screenshot.set(false);
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                // Countdown timer for retry (10 seconds)
                                for seconds_remaining in (1..=10).rev() {
                                    status.set(format!(
                                        "❌ Connection failed: {} - Retrying in {}s...",
                                        e, seconds_remaining
                                    ));
                                    screenshot_status.set(format!(
                                        "⏳ Waiting for device... ({}/10)",
                                        11 - seconds_remaining
                                    ));
                                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                }
                                // Continue to retry loop
                            }
                        }
                    }
                });

                // If we reach here, connection was successful - break the retry loop
                break;
            }
        });
    });

    // Initialize game automation on first render
    use_effect(move || {
        let debug_mode = *DEBUG_MODE.get().unwrap_or(&false);
        // Clone signals for use in async context
        let mut automation_command_tx_clone = automation_command_tx;
        let mut automation_state_clone = automation_state;
        let mut screenshot_counter_clone = screenshot_counter;
        let mut screenshot_data_clone = screenshot_data;
        let mut screenshot_bytes_clone = screenshot_bytes;
        let mut screenshot_status_clone = screenshot_status;
        let mut timed_tap_countdown_clone = timed_tap_countdown;
        let mut timed_events_list_clone = timed_events_list;
        let mut is_paused_by_touch_clone = is_paused_by_touch;
        let mut touch_timeout_remaining_clone = touch_timeout_remaining;
        let mut status_clone = status;
        let mut device_info_clone = device_info;

        spawn(async move {
            // Create automation channels
            let (cmd_tx, cmd_rx, event_tx, mut event_rx) = create_automation_channels();

            // Store command sender for GUI controls
            automation_command_tx_clone.set(Some(cmd_tx.clone()));

            // Start automation task
            let mut automation = GameAutomation::new(cmd_rx, event_tx, debug_mode);

            // Retry loop for automation ADB initialization
            loop {
                match automation.initialize_adb().await {
                    Ok(_) => {
                        if debug_mode {
                            println!("✅ Automation ADB initialized successfully");
                        }
                        break; // Success - exit retry loop
                    }
                    Err(e) => {
                        if debug_mode {
                            println!(
                                "❌ Failed to initialize automation ADB: {} - will retry...",
                                e
                            );
                        }
                        // Wait 10 seconds before retrying
                        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                        // Loop will retry
                    }
                }
            }

            // Spawn automation FSM loop
            let _automation_task = spawn(async move {
                automation.run().await;
            });

            // Auto-start automation when initialized
            let auto_start_tx = cmd_tx.clone();
            spawn(async move {
                // Small delay to ensure automation FSM is ready
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                let _ = auto_start_tx.send(AutomationCommand::Start).await;
            });

            // Event listener loop
            spawn(async move {
                let mut screenshot_count = 0;
                while let Some(event) = event_rx.recv().await {
                    match event {
                        AutomationEvent::ScreenshotReady(bytes) => {
                            // Update GUI with new screenshot from automation
                            let counter_val = screenshot_counter_clone.with_mut(|c| {
                                *c += 1;
                                *c
                            });
                            // Move base64 encoding to background thread for automation screenshots too
                            let bytes_clone = bytes.clone();
                            let base64_string =
                                tokio::task::spawn_blocking(move || base64_encode(&bytes_clone))
                                    .await
                                    .unwrap_or_else(|_| "".to_string());
                            screenshot_data_clone.set(Some(base64_string));
                            screenshot_bytes_clone.set(Some(bytes));
                            screenshot_status_clone
                                .set(format!("🤖 Automation screenshot #{}", counter_val));
                        }
                        AutomationEvent::ScreenshotTaken(screenshot_data, duration_ms) => {
                            screenshot_count += 1;
                            if debug_mode {
                                println!(
                                    "📸 GUI: Screenshot #{} captured ({} bytes) in {}ms",
                                    screenshot_count,
                                    screenshot_data.len(),
                                    duration_ms
                                );
                            }

                            // Update screenshot display with timing information
                            screenshot_status_clone.set(format!(
                                "📸 Automated screenshot #{} ({}ms)",
                                screenshot_count, duration_ms
                            ));

                            // Update the actual screenshot data
                            if let Ok(_image) = image::load_from_memory(&screenshot_data) {
                                screenshot_data_clone.set(Some(base64_encode(&screenshot_data)));
                                screenshot_bytes_clone.set(Some(screenshot_data));
                            }
                        }
                        AutomationEvent::StateChanged(new_state) => {
                            automation_state_clone.set(new_state);
                        }
                        AutomationEvent::Error(error) => {
                            if debug_mode {
                                println!("🤖 Automation error: {}", error);
                            }
                            screenshot_status_clone.set(format!("🤖 Automation error: {}", error));
                        }
                        AutomationEvent::DeviceDisconnected(error) => {
                            if debug_mode {
                                println!("🔌 Device disconnected: {}", error);
                            }

                            // Clear device info to hide "Connected" status
                            device_info_clone.set(None);

                            // Update GUI to reflect disconnection
                            screenshot_data_clone.set(None); // Clear screenshot
                            screenshot_bytes_clone.set(None); // Clear screenshot bytes

                            // Update status messages with clear indication
                            screenshot_status_clone.set(format!(
                                "🔌 USB DISCONNECTED: {} - Please reconnect the device",
                                error
                            ));
                            status_clone
                                .set("🔌 Device Disconnected - Automation Paused".to_string());

                            // Note: The automation FSM will automatically pause when disconnect is detected
                            // User can reconnect USB and resume automation manually
                        }
                        AutomationEvent::TemplatesUpdated(templates) => {
                            if debug_mode {
                                println!(
                                    "🔄 Templates updated: {} files found: {:?}",
                                    templates.len(),
                                    templates
                                );
                            }
                            screenshot_status_clone.set(format!(
                                "🔄 Templates updated: {} files found",
                                templates.len()
                            ));
                        }
                        AutomationEvent::TimedTapExecuted(id, x, y) => {
                            if debug_mode {
                                println!("🕒 GUI: Timed tap '{}' executed at ({},{})", id, x, y);
                            }
                            screenshot_status_clone
                                .set(format!("🕒 Timed tap '{}' executed at ({},{})", id, x, y));
                        }
                        AutomationEvent::TimedEventsListed(events) => {
                            // Store the events list for GUI display
                            timed_events_list_clone.set(events.clone());

                            if debug_mode {
                                println!("📋 GUI: Listed {} timed events", events.len());
                                for event in &events {
                                    println!(
                                        "  - {}: {:?} every {}s",
                                        event.id,
                                        event.event_type,
                                        event.interval.as_secs()
                                    );
                                }
                            }
                            screenshot_status_clone
                                .set(format!("📋 {} timed events configured", events.len()));
                        }
                        AutomationEvent::TimedTapCountdown(id, seconds) => {
                            // Update countdown signal for GUI display
                            timed_tap_countdown_clone.set(Some((id.clone(), seconds)));

                            if debug_mode && seconds % 30 == 0 {
                                // Only show every 30 seconds to avoid spam
                                println!(
                                    "🕒 Countdown: {} in {}s ({:.1}min)",
                                    id,
                                    seconds,
                                    seconds as f32 / 60.0
                                );
                            }
                        }
                        AutomationEvent::TimedEventExecuted(id) => {
                            if debug_mode {
                                println!("⚡ GUI: Timed event '{}' executed", id);
                            }
                            // Could add more GUI feedback here if needed
                        }
                        AutomationEvent::NextTimedEvent(id, seconds) => {
                            if debug_mode {
                                println!("⏱️ GUI: Next event '{}' in {}s", id, seconds);
                            }
                            // Could use this for a general event countdown if needed
                        }
                        AutomationEvent::ManualActivityDetected(is_active, remaining_seconds) => {
                            // Update the pause state signal
                            is_paused_by_touch_clone.set(is_active);
                            touch_timeout_remaining_clone.set(remaining_seconds);
                        }
                        AutomationEvent::ReconnectionAttempt(seconds_remaining) => {
                            if debug_mode && seconds_remaining % 5 == 0 {
                                println!("🔄 Reconnection attempt in {}s", seconds_remaining);
                            }
                            screenshot_status_clone.set(format!(
                                "🔌 Device disconnected - Retrying connection in {}s...",
                                seconds_remaining
                            ));
                        }
                        AutomationEvent::DeviceReconnected => {
                            if debug_mode {
                                println!("✅ Device reconnected successfully!");
                            }
                            screenshot_status_clone
                                .set("✅ Device reconnected! Restoring connection...".to_string());
                            status_clone.set("✅ Device Reconnected - Auto-Resuming".to_string());

                            // Restore device info by reconnecting in GUI
                            spawn(async move {
                                match AdbBackend::connect_first().await {
                                    Ok(client) => {
                                        let (sx, sy) = client.screen_dimensions();
                                        device_info_clone.set(Some((
                                            client.device_name().to_string(),
                                            client.transport_id(),
                                            sx,
                                            sy,
                                        )));
                                        status_clone.set("✅ Connected".to_string());
                                        screenshot_status_clone.set(
                                            "✅ Device reconnected! Automation ready.".to_string(),
                                        );

                                        if debug_mode {
                                            println!("✅ GUI device info restored");
                                        }

                                        // Take a fresh screenshot to show device is working
                                        match client.screen_capture_bytes().await {
                                            Ok(bytes) => {
                                                let bytes_clone = bytes.clone();
                                                let base64_string =
                                                    tokio::task::spawn_blocking(move || {
                                                        base64_encode(&bytes_clone)
                                                    })
                                                    .await
                                                    .unwrap_or_else(|_| "".to_string());

                                                screenshot_data_clone.set(Some(base64_string));
                                                screenshot_bytes_clone.set(Some(bytes));
                                                screenshot_status_clone.set(
                                                    "✅ Reconnected - Automation auto-resumed!"
                                                        .to_string(),
                                                );
                                            }
                                            Err(e) => {
                                                if debug_mode {
                                                    println!(
                                                        "⚠️ Failed to take reconnection screenshot: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        if debug_mode {
                                            println!("❌ Failed to restore GUI connection: {}", e);
                                        }
                                        screenshot_status_clone
                                            .set(format!("❌ Failed to restore connection: {}", e));
                                    }
                                }
                            });
                        }
                    }
                }
            });
        });
    });

    // Prepare compact status display variables
    let current_status = status.read().clone();
    let status_label = if current_status.contains("Connected") {
        "Connected"
    } else if current_status.contains("Error") {
        "Error"
    } else {
        current_status.as_str()
    };
    let status_style = if current_status.contains("Connected") {
        "background: #1f5130; color: #48ff9b; border: 1px solid #48ff9b; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; letter-spacing: 0.5px; font-weight: 600;"
    } else if current_status.contains("Error") {
        "background: #5a1f1f; color: #ff6262; border: 1px solid #ff6262; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; letter-spacing: 0.5px; font-weight: 600;"
    } else {
        "background: #5a4b1f; color: #ffd857; border: 1px solid #ffd857; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; letter-spacing: 0.5px; font-weight: 600;"
    };
    let runtime_days_value = *runtime_days.read();

    rsx! {
        // Main app container: vertical layout, fills viewport
        div {
            style: "height:97vh; display:flex; flex-direction:column; background:linear-gradient(135deg,#667eea 0%,#764ba2 100%); color:white; box-sizing:border-box;",
            // Scrollable content area
            div {
                style: "flex:1; overflow:auto; padding:8px;",
                // Horizontal split: left (info/actions), right (screenshot)
                div { style: "display:flex; gap:14px; align-items:flex-start;",
                    // Left column: device info, actions, credits
                    div { style: "flex:1; min-width:0; display:flex; flex-direction:column; gap:10px;",
                        // Device info and actions (only if device connected)
                        if let Some((name, transport_id_opt, screen_x, screen_y)) = device_info.read().clone() {
                            // Device metadata panel
                            DeviceInfo { name: name.clone(), transport_id: transport_id_opt, screen_x: screen_x, screen_y: screen_y, status_style: status_style.to_string(), status_label: status_label.to_string(), runtime_days: runtime_days_value }

                            // Action buttons (screenshot, save, exit, etc) - automation controls will show pause state
                            Actions {
                                screenshot_status: screenshot_status,
                                screenshot_bytes: screenshot_bytes,
                                auto_update_on_touch: auto_update_on_touch,
                                select_box: select_box,
                                automation_state: automation_state,
                                automation_command_tx: automation_command_tx,
                                timed_tap_countdown: timed_tap_countdown,
                                timed_events_list: timed_events_list,
                                is_paused_by_touch: is_paused_by_touch,  // Pass touch pause state to Actions
                                touch_timeout_remaining: touch_timeout_remaining,  // Pass countdown timer
                                hover_tap_preview: hover_tap_preview
                            }
                        } else {
                            // Fallback panel if no device is connected - show live status updates
                            div { style: "background:rgba(255,255,255,0.1); backdrop-filter:blur(10px); padding:20px; border-radius:15px; margin-bottom:20px; border:1px solid rgba(255,255,255,0.2);",
                                h2 { style: "margin-top:0; color:#ffb347;", "⚠️ No Device Connected" }

                                // Show current connection status with countdown
                                div { style: "background:rgba(0,0,0,0.3); padding:15px; border-radius:10px; margin:15px 0;",
                                    p { style: "font-size:1.2em; margin:0; text-align:center; font-weight:600;",
                                        "{current_status}"
                                    }
                                }

                                // Show screenshot status with progress indicator
                                if !screenshot_status.read().is_empty() {
                                    div { style: "background:rgba(0,0,0,0.2); padding:12px; border-radius:8px; margin:10px 0;",
                                        p { style: "font-size:1em; margin:0; text-align:center; color:#ffd857;",
                                            "{screenshot_status.read()}"
                                        }
                                    }
                                }

                                // Helpful message
                                p { style: "font-size:0.95em; margin:15px 0; text-align:center; color:rgba(255,255,255,0.7);",
                                    "Connect your Android device via USB with ADB debugging enabled"
                                }

                                button { style: "background:linear-gradient(45deg,#dc3545,#e74c3c); color:white; padding:15px 25px; border:none; border-radius:10px; cursor:pointer; font-size:1.1em; font-weight:bold; min-width:150px;", onclick: move |_| { std::thread::spawn(|| std::process::exit(0)); }, "🚪 Exit Application" }
                            }
                        }
                        // Credits/footer
                        div { style: "margin-top:4px; text-align:left; font-size:0.7em; opacity:0.75; letter-spacing:0.5px;", "Built with Rust 🦀 and Dioxus ⚛️" }
                    }
                    // Right column: screenshot panel (image, gestures)
                    screenshot_panel { screenshot_status: screenshot_status, screenshot_data: screenshot_data, screenshot_bytes: screenshot_bytes, device_info: device_info, device_coords: device_coords, mouse_coords: mouse_coords, is_loading_screenshot: is_loading_screenshot, auto_update_on_touch: auto_update_on_touch, is_swiping: is_swiping, swipe_start: swipe_start, swipe_end: swipe_end, calculate_device_coords: calculate_device_coords, select_box: select_box, selection_start: selection_start, selection_end: selection_end, tap_markers: tap_markers, screenshot_counter: screenshot_counter, automation_command_tx: automation_command_tx, hover_tap_preview: hover_tap_preview }
                }
            }
        }
    }
}
