// gui/components/actions.rs
use crate::adb::AdbBackend;
use crate::game_automation::{AutomationCommand, GameState};
use crate::gui::util::base64_encode;
use dioxus::prelude::*;
use tokio::sync::mpsc;

#[derive(Props, PartialEq, Clone)]
pub struct ActionsProps {
    pub name: String,
    pub is_loading: Signal<bool>,
    pub screenshot_status: Signal<String>,
    pub screenshot_data: Signal<Option<String>>,
    pub screenshot_bytes: Signal<Option<Vec<u8>>>,
    pub auto_update_on_touch: Signal<bool>,
    pub select_box: Signal<bool>, // new signal for select box
    pub use_rust_impl: bool,
    pub screenshot_counter: Signal<u64>, // GUI-level counter
    pub automation_state: Signal<GameState>,
    pub automation_command_tx: Signal<Option<mpsc::Sender<AutomationCommand>>>,
    pub automation_interval: Signal<u64>,
    pub timed_tap_countdown: Signal<Option<(String, u64)>>, // (id, seconds_remaining)
}

#[component]
pub fn Actions(props: ActionsProps) -> Element {
    let mut is_loading = props.is_loading;
    let mut screenshot_status = props.screenshot_status;
    let mut screenshot_data = props.screenshot_data;
    let mut screenshot_bytes = props.screenshot_bytes;
    let mut auto_update_on_touch = props.auto_update_on_touch;
    let mut select_box = props.select_box;
    let name = props.name.clone();
    let use_rust_impl = props.use_rust_impl;
    let mut screenshot_counter = props.screenshot_counter;
    let automation_state = props.automation_state;
    let automation_command_tx = props.automation_command_tx;
    let mut automation_interval = props.automation_interval;
    let timed_tap_countdown = props.timed_tap_countdown;

    // Toggle between Manual and Auto modes
    let mut mode_is_auto = use_signal(|| false);
    rsx! {
        div { style: "background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 15px; border-radius: 15px; margin-bottom: 15px; border: 1px solid rgba(255,255,255,0.2);",
            // Mode Toggle Header
            div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 15px;",
                h2 { style: "margin: 0; color: #87ceeb; font-size: 1.1em;", "🎮 Controls" }
                div { style: "display: flex; border-radius: 8px; overflow: hidden; border: 1px solid rgba(255,255,255,0.3);",
                    button {
                        style: if !*mode_is_auto.read() { "background: linear-gradient(45deg, #28a745, #20c997); color: white; padding: 6px 16px; border: none; cursor: pointer; font-size: 0.9em; font-weight: bold;" } else { "background: rgba(255,255,255,0.1); color: #ccc; padding: 6px 16px; border: none; cursor: pointer; font-size: 0.9em;" },
                        onclick: move |_| mode_is_auto.set(false),
                        "📱 Manual"
                    }
                    button {
                        style: if *mode_is_auto.read() { "background: linear-gradient(45deg, #6f42c1, #563d7c); color: white; padding: 6px 16px; border: none; cursor: pointer; font-size: 0.9em; font-weight: bold;" } else { "background: rgba(255,255,255,0.1); color: #ccc; padding: 6px 16px; border: none; cursor: pointer; font-size: 0.9em;" },
                        onclick: move |_| mode_is_auto.set(true),
                        "🤖 Auto"
                    }
                }
            }

            // Content based on mode
            if !*mode_is_auto.read() {
                // Manual Mode Controls
                div { style: "display: flex; flex-direction: column; gap: 12px;",
                    // Screenshot button
                    button { style: if *is_loading.read() { "background:linear-gradient(45deg,#ff6b35,#f7931e); color:white; padding:12px 20px; border:none; border-radius:8px; cursor:wait; font-size:1em; font-weight:bold;" } else { "background:linear-gradient(45deg,#28a745,#20c997); color:white; padding:12px 20px; border:none; border-radius:8px; cursor:pointer; font-size:1em; font-weight:bold;" },
                        onclick: move |_| {
                            if *is_loading.read() { return; }
                            let name_clone = name.clone();
                            is_loading.set(true);
                            screenshot_status.set("📸 Taking screenshot...".to_string());
                            spawn(async move {
                                let start = std::time::Instant::now();
                                let result = async move {
                                    match AdbBackend::new_with_device(&name_clone, use_rust_impl).await {
                                        Ok(client) => match client.screen_capture_bytes().await {
                                            Ok(bytes) => Ok(bytes),
                                            Err(e) => Err(format!("Screenshot failed: {}", e)),
                                        },
                                        Err(e) => Err(format!("ADB connection failed: {}", e)),
                                    }
                                }.await;
                                match result {
                                    Ok(bytes) => {
                                        let duration_ms = start.elapsed().as_millis();
                                        let counter_val = screenshot_counter.with_mut(|c| { *c += 1; *c });
                                        // Move base64 encoding to background thread
                                        let bytes_clone = bytes.clone();
                                        let b64 = tokio::task::spawn_blocking(move || {
                                            base64_encode(&bytes_clone)
                                        }).await.unwrap_or_else(|_| "".to_string());
                                        screenshot_data.set(Some(b64));
                                        screenshot_bytes.set(Some(bytes));
                                        screenshot_status.set(format!("✅ Screenshot #{} captured in {}ms", counter_val, duration_ms));
                                    }
                                    Err(e) => screenshot_status.set(format!("❌ {}", e)),
                                }
                                is_loading.set(false);
                            });
                        },
                        if *is_loading.read() { "📸 Taking..." } else { "📸 Screenshot" }
                    }

                    // Compact options row
                    div { style: "display: flex; gap: 15px; align-items: center; justify-content: center; flex-wrap: wrap;",
                        div { style: "display: flex; align-items: center; gap: 6px;",
                            input {
                                r#type: "checkbox",
                                id: "auto-update-checkbox",
                                checked: *auto_update_on_touch.read(),
                                onchange: move |evt| {
                                    let checked = evt.value().parse().unwrap_or(false);
                                    auto_update_on_touch.set(checked);
                                    if checked { select_box.set(false); }
                                },
                                style: "width: 14px; height: 14px; cursor: pointer;"
                            }
                            label { r#for: "auto-update-checkbox", style: "font-size: 0.85em; cursor: pointer; user-select: none;", "📱 Update on tap" }
                        }
                        div { style: "display: flex; align-items: center; gap: 6px;",
                            input {
                                r#type: "checkbox",
                                id: "select-box-checkbox",
                                checked: *select_box.read(),
                                onchange: move |evt| {
                                    let checked = evt.value().parse().unwrap_or(false);
                                    select_box.set(checked);
                                    if checked { auto_update_on_touch.set(false); }
                                },
                                style: "width: 14px; height: 14px; cursor: pointer;"
                            }
                            label { r#for: "select-box-checkbox", style: "font-size: 0.85em; cursor: pointer; user-select: none;", "🟦 Select box" }
                        }
                    }

                    // Action buttons row
                    div { style: "display: flex; gap: 8px; justify-content: center; flex-wrap: wrap;",
                        if screenshot_bytes.read().is_some() {
                            button { style: "background: linear-gradient(45deg, #6f42c1, #563d7c); color: white; padding: 8px 16px; border: none; border-radius: 6px; cursor: pointer; font-size: 0.9em; font-weight: bold;",
                                onclick: move |_| {
                                    if let Some(bytes) = screenshot_bytes.read().clone() {
                                        spawn(async move {
                                            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                                            let filename = format!("screenshot_{}.png", ts);
                                            match tokio::fs::write(&filename, &bytes).await {
                                                Ok(_) => screenshot_status.set(format!("✅ Screenshot saved to {}", filename)),
                                                Err(e) => screenshot_status.set(format!("❌ Failed to save: {}", e)),
                                            }
                                        });
                                    }
                                },
                                "💾 Save"
                            }
                        }
                        button { style: "background: linear-gradient(45deg, #dc3545, #e74c3c); color: white; padding: 8px 16px; border: none; border-radius: 6px; cursor: pointer; font-size: 0.9em; font-weight: bold;",
                            onclick: move |_| { std::thread::spawn(|| std::process::exit(0)); },
                            "🚪 Exit"
                        }
                    }
                }
            } else {
                // Auto Mode Controls (Game Automation)
                div { style: "display: flex; flex-direction: column; gap: 12px;",
                    // Status and control buttons
                    div { style: "display: flex; gap: 8px; flex-wrap: wrap; align-items: center; justify-content: center;",
                        // Automation state indicator
                        div {
                            style: match *automation_state.read() {
                                GameState::Idle => "background: #666; color: white; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;",
                                GameState::WaitingForScreenshot => "background: #17a2b8; color: white; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;",
                                GameState::Analyzing => "background: #ffc107; color: black; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;",
                                GameState::Acting => "background: #28a745; color: white; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;",
                                GameState::Paused => "background: #fd7e14; color: white; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;",
                            },
                            {format!("{:?}", *automation_state.read())}
                        }

                        // Control buttons
                        if *automation_state.read() == GameState::Idle || *automation_state.read() == GameState::Paused {
                            button { style: "background: linear-gradient(45deg, #28a745, #20c997); color: white; padding: 6px 12px; border: none; border-radius: 6px; cursor: pointer; font-size: 0.85em; font-weight: bold;",
                                onclick: move |_| {
                                    if let Some(tx) = automation_command_tx.read().as_ref() {
                                        let tx = tx.clone();
                                        spawn(async move {
                                            let cmd = if *automation_state.peek() == GameState::Paused {
                                                AutomationCommand::Resume
                                            } else {
                                                AutomationCommand::Start
                                            };
                                            let _ = tx.send(cmd).await;
                                        });
                                    }
                                },
                                if *automation_state.read() == GameState::Paused { "▶️ Resume" } else { "🚀 Start" }
                            }
                        }
                        if matches!(*automation_state.read(), GameState::WaitingForScreenshot | GameState::Analyzing | GameState::Acting) {
                            button { style: "background: linear-gradient(45deg, #fd7e14, #f39c12); color: white; padding: 6px 12px; border: none; border-radius: 6px; cursor: pointer; font-size: 0.85em; font-weight: bold;",
                                onclick: move |_| {
                                    if let Some(tx) = automation_command_tx.read().as_ref() {
                                        let tx = tx.clone();
                                        spawn(async move {
                                            let _ = tx.send(AutomationCommand::Pause).await;
                                        });
                                    }
                                },
                                "⏸️ Pause"
                            }
                        }
                        if *automation_state.read() != GameState::Idle {
                            button { style: "background: linear-gradient(45deg, #dc3545, #e74c3c); color: white; padding: 6px 12px; border: none; border-radius: 6px; cursor: pointer; font-size: 0.85em; font-weight: bold;",
                                onclick: move |_| {
                                    if let Some(tx) = automation_command_tx.read().as_ref() {
                                        let tx = tx.clone();
                                        spawn(async move {
                                            let _ = tx.send(AutomationCommand::Stop).await;
                                        });
                                    }
                                },
                                "⏹️ Stop"
                            }
                        }
                        button { style: "background: linear-gradient(45deg, #6f42c1, #563d7c); color: white; padding: 6px 12px; border: none; border-radius: 6px; cursor: pointer; font-size: 0.85em; font-weight: bold;",
                            onclick: move |_| {
                                if let Some(tx) = automation_command_tx.read().as_ref() {
                                    let tx = tx.clone();
                                    spawn(async move {
                                        let _ = tx.send(AutomationCommand::TakeScreenshot).await;
                                    });
                                }
                            },
                            "📸 Shot"
                        }
                    }

                    // Timed Tap Countdown Display
                    if let Some((tap_id, seconds_remaining)) = timed_tap_countdown.read().clone() {
                        div { style: "background: rgba(0,0,0,0.2); border-radius: 8px; padding: 8px 12px; border: 1px solid rgba(255,255,255,0.2);",
                            div { style: "display: flex; align-items: center; justify-content: space-between; gap: 10px;",
                                div { style: "display: flex; align-items: center; gap: 6px;",
                                    span { style: "font-size: 0.85em; color: #87ceeb;", "🕒 Next Tap:" }
                                    span { style: "font-size: 0.8em; color: #ccc;", "{tap_id}" }
                                }
                                div { style: "font-family: monospace; font-weight: bold; color: #ffd857;",
                                    if seconds_remaining < 60 {
                                        span { style: "font-size: 0.9em;", "{seconds_remaining}s" }
                                    } else {
                                        span { style: "font-size: 0.9em;", "{seconds_remaining / 60}m {seconds_remaining % 60}s" }
                                    }
                                }
                            }
                            if seconds_remaining <= 10 {
                                div { style: "margin-top: 4px; height: 2px; background: linear-gradient(to right, #ff4444, #ff8888); border-radius: 1px; animation: pulse 1s infinite;",
                                }
                            } else if seconds_remaining <= 60 {
                                div { style: "margin-top: 4px; height: 2px; background: linear-gradient(to right, #ffaa00, #ffdd44); border-radius: 1px;",
                                }
                            } else {
                                div { style: "margin-top: 4px; height: 2px; background: linear-gradient(to right, #28a745, #48ff9b); border-radius: 1px;",
                                }
                            }
                        }
                    }

                    // Interval control
                    div { style: "display: flex; gap: 8px; align-items: center; justify-content: center;",
                        label { style: "font-size: 0.85em; color: #ccc;", "Interval:" }
                        input {
                            r#type: "number",
                            min: "5",
                            max: "300",
                            value: "{automation_interval}",
                            style: "width: 60px; padding: 3px 6px; border: 1px solid rgba(255,255,255,0.3); border-radius: 4px; background: rgba(255,255,255,0.1); color: white; font-size: 0.85em;",
                            onchange: move |evt| {
                                if let Ok(seconds) = evt.value().parse::<u64>() {
                                    automation_interval.set(seconds);
                                    if let Some(tx) = automation_command_tx.read().as_ref() {
                                        let tx = tx.clone();
                                        spawn(async move {
                                            let _ = tx.send(AutomationCommand::UpdateInterval(seconds)).await;
                                        });
                                    }
                                }
                            }
                        }
                        span { style: "font-size: 0.85em; color: #ccc;", "sec" }
                    }

                    // Exit button
                    div { style: "display: flex; justify-content: center;",
                        button { style: "background: linear-gradient(45deg, #dc3545, #e74c3c); color: white; padding: 8px 16px; border: none; border-radius: 6px; cursor: pointer; font-size: 0.9em; font-weight: bold;",
                            onclick: move |_| { std::thread::spawn(|| std::process::exit(0)); },
                            "🚪 Exit"
                        }
                    }
                }
            }
        }
    }
}
