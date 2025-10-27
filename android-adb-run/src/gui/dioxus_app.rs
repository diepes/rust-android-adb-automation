use crate::AdbBackend;
use crate::gui::components::interaction_info::InteractionInfo;
use crate::gui::components::{
    actions::Actions, device_info::DeviceInfo, header::Header, screenshot_panel::screenshot_panel,
};
use crate::gui::util::base64_encode;
use dioxus::prelude::*;

pub fn run_gui() {
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
    let mouse_coords = use_signal(|| None::<(i32, i32)>);
    let device_coords = use_signal(|| None::<(u32, u32)>);
    let auto_update_on_touch = use_signal(|| true);
    let select_box = use_signal(|| false);
    let mut is_loading_screenshot = use_signal(|| false);

    let selection_start = use_signal(|| None::<dioxus::html::geometry::ElementPoint>);
    let selection_end = use_signal(|| None::<dioxus::html::geometry::ElementPoint>);

    // Swipe gesture state
    let is_swiping = use_signal(|| false);
    let swipe_start = use_signal(|| None::<(u32, u32)>);
    let swipe_end = use_signal(|| None::<(u32, u32)>);

    let tap_markers = use_signal(|| Vec::<dioxus::html::geometry::ElementPoint>::new());

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

    // Initialize ADB connection on first render
    use_effect(move || {
        spawn(async move {
            match AdbBackend::connect_first().await {
                Ok(client) => {
                    let (sx, sy) = client.screen_dimensions();
                    device_info.set(Some((
                        client.device_name().to_string(),
                        client.transport_id(),
                        sx,
                        sy,
                    )));
                    status.set(format!(
                        "Connected via {}",
                        std::env::var("ADB_IMPL").unwrap_or_else(|_| "rust".to_string())
                    ));
                    is_loading_screenshot.set(true);
                    screenshot_status.set("📸 Taking initial screenshot...".to_string());
                    match client.screen_capture().await {
                        Ok(image_cap) => {
                            let base64_string = base64_encode(&image_cap.bytes);
                            screenshot_data.set(Some(base64_string));
                            screenshot_bytes.set(Some(image_cap.bytes.clone()));
                            screenshot_status.set(format!(
                                "✅ Initial screenshot #{} ({}ms)",
                                image_cap.index, image_cap.duration_ms
                            ));
                            is_loading_screenshot.set(false);
                        }
                        Err(e) => {
                            screenshot_status.set(format!("❌ Initial screenshot failed: {}", e));
                            is_loading_screenshot.set(false);
                        }
                    }
                }
                Err(e) => status.set(format!("Error: {e}")),
            }
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
                            Actions { name: name.clone(), is_loading: is_loading_screenshot, screenshot_status: screenshot_status, screenshot_data: screenshot_data, screenshot_bytes: screenshot_bytes, auto_update_on_touch: auto_update_on_touch, select_box: select_box }
                            // Interaction info (tap/swipe coordinates, status)
                            InteractionInfo { device_coords: device_coords, screenshot_status: screenshot_status }
                        } else {
                            // Fallback panel if no device is connected
                            div { style: "background:rgba(255,255,255,0.1); backdrop-filter:blur(10px); padding:20px; border-radius:15px; margin-bottom:20px; border:1px solid rgba(255,255,255,0.2);",
                                h2 { style: "margin-top:0; color:#ffb347;", "⚠️ No Device Connected" }
                                p { style: "font-size:1.1em; margin:15px 0; text-align:center;", "{fallback_message}" }
                                button { style: "background:linear-gradient(45deg,#dc3545,#e74c3c); color:white; padding:15px 25px; border:none; border-radius:10px; cursor:pointer; font-size:1.1em; font-weight:bold; min-width:150px;", onclick: move |_| -> () { std::process::exit(0); }, "🚪 Exit Application" }
                            }
                        }
                        // Credits/footer
                        div { style: "margin-top:4px; text-align:left; font-size:0.7em; opacity:0.75; letter-spacing:0.5px;", "Built with Rust 🦀 and Dioxus ⚛️" }
                    }
                    // Right column: screenshot panel (image, gestures)
                    screenshot_panel { screenshot_status: screenshot_status, screenshot_data: screenshot_data, screenshot_bytes: screenshot_bytes, device_info: device_info, device_coords: device_coords, mouse_coords: mouse_coords, is_loading_screenshot: is_loading_screenshot, auto_update_on_touch: auto_update_on_touch, is_swiping: is_swiping, swipe_start: swipe_start, swipe_end: swipe_end, calculate_device_coords: calculate_device_coords, select_box: select_box, selection_start: selection_start, selection_end: selection_end, tap_markers: tap_markers }
                }
            }
        }
    }
}
