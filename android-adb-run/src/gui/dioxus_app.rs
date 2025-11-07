use crate::adb::AdbBackend;
use crate::game_automation::{
    AutomationCommand, AutomationEvent, GameAutomation, GameState, create_automation_channels,
};
use crate::gui::components::interaction_info::InteractionInfo;
use crate::gui::components::{
    actions::Actions, device_info::DeviceInfo, header::Header, screenshot_panel::screenshot_panel,
};
use crate::gui::util::base64_encode;
use dioxus::prelude::*;
use std::sync::OnceLock;
use tokio::sync::mpsc;

// Global state to store the ADB implementation choice
static USE_RUST_IMPL: OnceLock<bool> = OnceLock::new();

// Global state to store the debug mode choice
static DEBUG_MODE: OnceLock<bool> = OnceLock::new();

pub fn is_debug_mode() -> bool {
    *DEBUG_MODE.get().unwrap_or(&false)
}

pub fn run_gui(use_rust_impl: bool, debug_mode: bool) {
    USE_RUST_IMPL
        .set(use_rust_impl)
        .expect("USE_RUST_IMPL should only be set once");
    DEBUG_MODE
        .set(debug_mode)
        .expect("DEBUG_MODE should only be set once");

    use dioxus::desktop::{Config, WindowBuilder};
    let enable_borderless = true; // borderless window
    let config = Config::new().with_window(
        WindowBuilder::new()
            .with_title("Android ADB Automation")
            .with_decorations(!enable_borderless) // false => no native title/menu
            .with_resizable(true)
            .with_inner_size(dioxus::desktop::LogicalSize::new(1000, 700)),
    );
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(App);
}

#[component]
fn App() -> Element {
    use dioxus::desktop::use_window; // access desktop window for dragging
    let desktop = use_window();
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
    let automation_interval = use_signal(|| 30u64);
    let timed_tap_countdown = use_signal(|| None::<(String, u64)>); // (id, seconds_remaining)

    let selection_start = use_signal(|| None::<dioxus::html::geometry::ElementPoint>);
    let selection_end = use_signal(|| None::<dioxus::html::geometry::ElementPoint>);

    // Swipe gesture state
    let is_swiping = use_signal(|| false);
    let swipe_start = use_signal(|| None::<(u32, u32)>);
    let swipe_end = use_signal(|| None::<(u32, u32)>);

    let tap_markers = use_signal(Vec::<dioxus::html::geometry::ElementPoint>::new);

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
        let use_rust_impl = *USE_RUST_IMPL.get().unwrap_or(&true);
        spawn(async move {
            // Step 1: Look for devices (fast operation)
            status.set("🔍 Looking for devices...".to_string());

            let devices = match AdbBackend::list_devices(use_rust_impl).await {
                Ok(devices) if !devices.is_empty() => devices,
                Ok(_) => {
                    status.set("❌ No devices found".to_string());
                    return;
                }
                Err(e) => {
                    status.set(format!("❌ Error listing devices: {e}"));
                    return;
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
                let use_rust_impl = use_rust_impl;
                async move {
                    match AdbBackend::new_with_device(&device_name, use_rust_impl).await {
                        Ok(client) => {
                            // Step 4: Connection successful, update device info immediately
                            let (sx, sy) = client.screen_dimensions();
                            device_info.set(Some((
                                client.device_name().to_string(),
                                client.transport_id(),
                                sx,
                                sy,
                            )));
                            status.set(format!(
                                "✅ Connected via {}",
                                if use_rust_impl { "rust" } else { "shell" }
                            ));

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
                                                    "❌ Failed to encode screenshot".to_string(),
                                                );
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
                        }
                        Err(e) => {
                            status.set(format!("❌ Connection failed: {e}"));
                        }
                    }
                }
            });
        });
    });

    // Initialize game automation on first render
    use_effect(move || {
        let use_rust_impl = *USE_RUST_IMPL.get().unwrap_or(&true);
        let debug_mode = *DEBUG_MODE.get().unwrap_or(&false);
        // Clone signals for use in async context
        let mut automation_command_tx_clone = automation_command_tx.clone();
        let mut automation_state_clone = automation_state.clone();
        let mut automation_interval_clone = automation_interval.clone();
        let mut screenshot_counter_clone = screenshot_counter.clone();
        let mut screenshot_data_clone = screenshot_data.clone();
        let mut screenshot_bytes_clone = screenshot_bytes.clone();
        let mut screenshot_status_clone = screenshot_status.clone();
        let mut timed_tap_countdown_clone = timed_tap_countdown.clone();

        spawn(async move {
            // Create automation channels
            let (cmd_tx, cmd_rx, event_tx, mut event_rx) = create_automation_channels();

            // Store command sender for GUI controls
            automation_command_tx_clone.set(Some(cmd_tx));

            // Start automation task
            let mut automation = GameAutomation::new(cmd_rx, event_tx, debug_mode);
            if let Err(e) = automation.initialize_adb(use_rust_impl).await {
                if debug_mode {
                    println!("❌ Failed to initialize automation ADB: {}", e);
                }
                return;
            }

            // Spawn automation FSM loop
            let _automation_task = spawn(async move {
                automation.run().await;
            });

            // Event listener loop
            spawn(async move {
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
                        AutomationEvent::StateChanged(new_state) => {
                            automation_state_clone.set(new_state);
                        }
                        AutomationEvent::Error(error) => {
                            if debug_mode {
                                println!("🤖 Automation error: {}", error);
                            }
                            screenshot_status_clone.set(format!("🤖 Automation error: {}", error));
                        }
                        AutomationEvent::IntervalUpdate(seconds) => {
                            automation_interval_clone.set(seconds);
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
                            screenshot_status_clone.set(format!(
                                "🕒 Timed tap '{}' executed at ({},{})",
                                id, x, y
                            ));
                        }
                        AutomationEvent::TimedTapsListed(taps) => {
                            if debug_mode {
                                println!("📋 GUI: Listed {} timed taps", taps.len());
                                for tap in &taps {
                                    println!("  - {}: ({},{}) every {}min", 
                                        tap.id, tap.x, tap.y, tap.interval.as_secs() / 60);
                                }
                            }
                            screenshot_status_clone.set(format!(
                                "📋 {} timed taps configured",
                                taps.len()
                            ));
                        }
                        AutomationEvent::TimedTapCountdown(id, seconds) => {
                            // Update countdown signal for GUI display
                            timed_tap_countdown_clone.set(Some((id.clone(), seconds)));
                            
                            if debug_mode && seconds % 30 == 0 { // Only show every 30 seconds to avoid spam
                                println!("🕒 Countdown: {} in {}s ({:.1}min)", id, seconds, seconds as f32 / 60.0);
                            }
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
    // Use detailed error message (e.g. missing adb guidance) when status contains Error
    let fallback_message = if current_status.contains("Error") {
        current_status.clone()
    } else {
        "Please connect an Android device with ADB enabled, or use the CLI version.".to_string()
    };

    rsx! {
        // Main app container: vertical layout, fills viewport
        div { style: "height:97vh; display:flex; flex-direction:column; background:linear-gradient(135deg,#667eea 0%,#764ba2 100%); color:white; border:1px solid rgba(255,255,255,0.25); box-sizing:content-box;",
            // Scrollable content area
            div { style: "flex:1; overflow:auto; padding:8px;",
                // Horizontal split: left (info/actions), right (screenshot)
                div { style: "display:flex; gap:14px; align-items:flex-start;",
                    // Left column: header, device info, actions, interaction info, credits
                    div { style: "flex:1; min-width:0; display:flex; flex-direction:column; gap:10px;",
                        // App header bar (drag/close)
                        Header { on_drag: move |_| { let _ = desktop.window.drag_window(); }, on_close: move |_| { std::thread::spawn(|| std::process::exit(0)); } }
                        // Device info, actions, and interaction info (only if device connected)
                        if let Some((name, transport_id_opt, screen_x, screen_y)) = device_info.read().clone() {
                            // Device metadata panel
                            DeviceInfo { name: name.clone(), transport_id: transport_id_opt, screen_x: screen_x, screen_y: screen_y, status_style: status_style.to_string(), status_label: status_label.to_string() }
                            // Action buttons (screenshot, save, exit, etc)
                            Actions {
                                name: name.clone(),
                                is_loading: is_loading_screenshot,
                                screenshot_status: screenshot_status,
                                screenshot_data: screenshot_data,
                                screenshot_bytes: screenshot_bytes,
                                auto_update_on_touch: auto_update_on_touch,
                                select_box: select_box,
                                use_rust_impl: *USE_RUST_IMPL.get().unwrap_or(&true),
                                screenshot_counter: screenshot_counter,
                                automation_state: automation_state,
                                automation_command_tx: automation_command_tx,
                                automation_interval: automation_interval,
                                timed_tap_countdown: timed_tap_countdown
                            }
                            // Interaction info (tap/swipe coordinates, status)
                            InteractionInfo { device_coords: device_coords, screenshot_status: screenshot_status }
                        } else {
                            // Fallback panel if no device is connected
                            div { style: "background:rgba(255,255,255,0.1); backdrop-filter:blur(10px); padding:20px; border-radius:15px; margin-bottom:20px; border:1px solid rgba(255,255,255,0.2);",
                                h2 { style: "margin-top:0; color:#ffb347;", "⚠️ No Device Connected" }
                                p { style: "font-size:1.1em; margin:15px 0; text-align:center;", "{fallback_message}" }
                                button { style: "background:linear-gradient(45deg,#dc3545,#e74c3c); color:white; padding:15px 25px; border:none; border-radius:10px; cursor:pointer; font-size:1.1em; font-weight:bold; min-width:150px;", onclick: move |_| { std::thread::spawn(|| std::process::exit(0)); }, "🚪 Exit Application" }
                            }
                        }
                        // Credits/footer
                        div { style: "margin-top:4px; text-align:left; font-size:0.7em; opacity:0.75; letter-spacing:0.5px;", "Built with Rust 🦀 and Dioxus ⚛️" }
                    }
                    // Right column: screenshot panel (image, gestures)
                    screenshot_panel { screenshot_status: screenshot_status, screenshot_data: screenshot_data, screenshot_bytes: screenshot_bytes, device_info: device_info, device_coords: device_coords, mouse_coords: mouse_coords, is_loading_screenshot: is_loading_screenshot, auto_update_on_touch: auto_update_on_touch, is_swiping: is_swiping, swipe_start: swipe_start, swipe_end: swipe_end, calculate_device_coords: calculate_device_coords, select_box: select_box, selection_start: selection_start, selection_end: selection_end, tap_markers: tap_markers, use_rust_impl: *USE_RUST_IMPL.get().unwrap_or(&true), screenshot_counter: screenshot_counter }
                }
            }
        }
    }
}
