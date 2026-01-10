use crate::adb::AdbBackend;
use crate::game_automation::types::{DeviceInfo, TimedEvent};
use crate::game_automation::{AutomationCommand, GameState};
use crate::gui::components::{
    actions::Actions,
    device_info::DeviceInfo,
    screenshot_panel::{TapMarker, screenshot_panel},
};
use crate::gui::hooks::{use_automation_loop, use_device_loop, use_runtime_timer};
use crate::gui::util::calculate_device_coords;
use dioxus::html::geometry::ElementPoint;
use dioxus::prelude::*;
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, mpsc};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_YEAR: &str = env!("CARGO_PKG_VERSION"); // Placeholder, consider a build script for this

// Global state to store the debug mode choice
static DEBUG_MODE: OnceLock<bool> = OnceLock::new();

pub fn is_debug_mode() -> bool {
    *DEBUG_MODE.get().unwrap_or(&false)
}

#[derive(Clone)]
pub struct AppContext {
    pub screenshot_status: Signal<String>,
    pub screenshot_data: Signal<Option<String>>,
    pub screenshot_bytes: Signal<Option<Vec<u8>>>,
    pub device_info: Signal<Option<DeviceInfo>>,
    pub device_coords: Signal<Option<(u32, u32)>>,
    pub mouse_coords: Signal<Option<(i32, i32)>>,
    pub is_loading_screenshot: Signal<bool>,
    pub auto_update_on_touch: Signal<bool>,
    pub select_box: Signal<bool>,
    pub is_swiping: Signal<bool>,
    pub swipe_start: Signal<Option<(u32, u32)>>,
    pub swipe_end: Signal<Option<(u32, u32)>>,
    pub selection_start: Signal<Option<ElementPoint>>,
    pub selection_end: Signal<Option<ElementPoint>>,
    pub tap_markers: Signal<Vec<TapMarker>>,
    pub screenshot_counter: Signal<u64>,
    pub automation_state: Signal<GameState>,
    pub automation_command_tx: Signal<Option<mpsc::Sender<AutomationCommand>>>,
    pub timed_tap_countdown: Signal<Option<(String, u64)>>,
    pub timed_events_list: Signal<Vec<TimedEvent>>,
    pub is_paused_by_touch: Signal<bool>,
    pub touch_timeout_remaining: Signal<Option<u64>>,
    pub hover_tap_preview: Signal<Option<(u32, u32)>>,
    pub shared_adb_client: Signal<Option<Arc<Mutex<AdbBackend>>>>,
    pub calculate_device_coords: fn(ElementPoint, u32, u32) -> (u32, u32),
}

fn ensure_gui_environment() -> Result<(), String> {
    ensure_gui_environment_inner()
}

#[cfg(target_os = "linux")]
fn ensure_gui_environment_inner() -> Result<(), String> {
    let has_display =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();

    if has_display {
        Ok(())
    } else {
        Err("GUI launch requires an available X11/Wayland display on Linux".to_string())
    }
}

#[cfg(not(target_os = "linux"))]
fn ensure_gui_environment_inner() -> Result<(), String> {
    Ok(())
}

pub fn run_gui(debug_mode: bool) {
    DEBUG_MODE
        .set(debug_mode)
        .expect("DEBUG_MODE should only be set once");

    if let Err(message) = ensure_gui_environment() {
        eprintln!("❌ {message}");
        eprintln!("Hint: launch with --screenshot for CLI mode or set DISPLAY/WAYLAND_DISPLAY.");
        return;
    }

    use dioxus::desktop::{Config, WindowBuilder};
    let window_title = format!(
        "Android ADB Automation v{} (Build {})",
        APP_VERSION, BUILD_YEAR
    );
    let config = Config::new()
        .with_window(
            WindowBuilder::new()
                .with_title(window_title)
                .with_decorations(true)
                .with_resizable(true)
                .with_inner_size(dioxus::desktop::LogicalSize::new(1000, 700)),
        )
        .with_menu(None);

    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(App);
}

#[component]
fn App() -> Element {
    let status = use_signal(|| "Initializing...".to_string());
    let device_info = use_signal(|| None::<(String, Option<u32>, u32, u32)>);
    let screenshot_data = use_signal(|| None::<String>);
    let screenshot_bytes = use_signal(|| None::<Vec<u8>>);
    let screenshot_status = use_signal(|| "".to_string());
    let screenshot_counter = use_signal(|| 0u64);
    let mouse_coords = use_signal(|| None::<(i32, i32)>);
    let device_coords = use_signal(|| None::<(u32, u32)>);
    let auto_update_on_touch = use_signal(|| true);
    let select_box = use_signal(|| false);
    let is_loading_screenshot = use_signal(|| false);
    let shared_adb_client = use_signal(|| None::<Arc<Mutex<AdbBackend>>>);
    let force_update = use_signal(|| 0u32);

    let automation_state = use_signal(|| GameState::Idle);
    let automation_command_tx = use_signal(|| None::<mpsc::Sender<AutomationCommand>>);
    let timed_tap_countdown = use_signal(|| None::<(String, u64)>);
    let timed_events_list = use_signal(Vec::<TimedEvent>::new);
    let is_paused_by_touch = use_signal(|| false);
    let touch_timeout_remaining = use_signal(|| None::<u64>);

    let selection_start = use_signal(|| None::<dioxus::html::geometry::ElementPoint>);
    let selection_end = use_signal(|| None::<dioxus::html::geometry::ElementPoint>);
    let is_swiping = use_signal(|| false);
    let swipe_start = use_signal(|| None::<(u32, u32)>);
    let swipe_end = use_signal(|| None::<(u32, u32)>);
    let tap_markers = use_signal(Vec::<TapMarker>::new);
    let runtime_days = use_signal(|| 0.0f64);
    let hover_tap_preview = use_signal(|| None::<(u32, u32)>);

    // Initialize hooks for background tasks
    use_runtime_timer(runtime_days);
    use_device_loop(
        status,
        device_info,
        is_loading_screenshot,
        screenshot_status,
        screenshot_data,
        screenshot_bytes,
        screenshot_counter,
        shared_adb_client,
        force_update,
    );
    use_automation_loop(
        is_debug_mode(),
        automation_command_tx,
        automation_state,
        screenshot_counter,
        screenshot_data,
        screenshot_bytes,
        screenshot_status,
        timed_tap_countdown,
        timed_events_list,
        is_paused_by_touch,
        touch_timeout_remaining,
        device_info,
        status,
        shared_adb_client,
    );

    use_context_provider(|| AppContext {
        screenshot_status,
        screenshot_data,
        screenshot_bytes,
        device_info,
        device_coords,
        mouse_coords,
        is_loading_screenshot,
        auto_update_on_touch,
        select_box,
        is_swiping,
        swipe_start,
        swipe_end,
        selection_start,
        selection_end,
        tap_markers,
        screenshot_counter,
        automation_state,
        automation_command_tx,
        timed_tap_countdown,
        timed_events_list,
        is_paused_by_touch,
        touch_timeout_remaining,
        hover_tap_preview,
        shared_adb_client,
        calculate_device_coords,
    });

    let current_status = status.read().clone();
    let _update_trigger = force_update.read();
    let (status_label, status_style) = if current_status.contains("Connected") {
        (
            "Connected",
            "background: #1f5130; color: #48ff9b; border: 1px solid #48ff9b; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; letter-spacing: 0.5px; font-weight: 600;",
        )
    } else if current_status.contains("Error") {
        (
            "Error",
            "background: #5a1f1f; color: #ff6262; border: 1px solid #ff6262; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; letter-spacing: 0.5px; font-weight: 600;",
        )
    } else {
        (
            current_status.as_str(),
            "background: #5a4b1f; color: #ffd857; border: 1px solid #ffd857; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; letter-spacing: 0.5px; font-weight: 600;",
        )
    };
    let runtime_days_value = *runtime_days.read();

    rsx! {
        div {
            style: "height:97vh; display:flex; flex-direction:column; background:linear-gradient(135deg,#667eea 0%,#764ba2 100%); color:white; box-sizing:border-box;",
            div {
                style: "flex:1; overflow:auto; padding:8px;",
                div { style: "display:flex; gap:14px; align-items:flex-start;",
                    div { style: "flex:1; min-width:0; display:flex; flex-direction:column; gap:10px;",
                        if let Some((name, transport_id_opt, screen_x, screen_y)) = device_info.read().clone() {
                            DeviceInfo { name: name, transport_id: transport_id_opt, screen_x: screen_x, screen_y: screen_y, status_style: status_style.to_string(), status_label: status_label.to_string(), runtime_days: runtime_days_value }
                            Actions {}
                        } else {
                            div { style: "background:rgba(255,255,255,0.1); backdrop-filter:blur(10px); padding:20px; border-radius:15px; margin-bottom:20px; border:1px solid rgba(255,255,255,0.2);",
                                h2 { style: "margin-top:0; color:#ffb347;", "⚠️ No Device Connected" }
                                div { style: "background:rgba(0,0,0,0.3); padding:15px; border-radius:10px; margin:15px 0;",
                                    p { style: "font-size:1.2em; margin:0; text-align:center; font-weight:600;", "{current_status}" }
                                }
                                if !screenshot_status.read().is_empty() {
                                    div { style: "background:rgba(0,0,0,0.2); padding:12px; border-radius:8px; margin:10px 0;",
                                        p { style: "font-size:1em; margin:0; text-align:center; color:#ffd857;", "{screenshot_status.read()}" }
                                    }
                                }
                                p { style: "font-size:0.95em; margin:15px 0; text-align:center; color:rgba(255,255,255,0.7);",
                                    "Connect your Android device via USB with ADB debugging enabled"
                                }
                                button {
                                    style: "background:linear-gradient(45deg,#dc3545,#e74c3c); color:white; padding:15px 25px; border:none; border-radius:10px; cursor:pointer; font-size:1.1em; font-weight:bold; min-width:150px;",
                                    onclick: move |_| {
                                        tokio::spawn(async {
                                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                                            std::process::exit(0);
                                        });
                                    },
                                    "🚪 Exit Application"
                                }
                            }
                        }
                        div { style: "margin-top:4px; text-align:left; font-size:0.7em; opacity:0.75; letter-spacing:0.5px;", "Built with Rust 🦀 and Dioxus ⚛️" }
                    }
                    screenshot_panel {}
                }
            }
        }
    }
}
