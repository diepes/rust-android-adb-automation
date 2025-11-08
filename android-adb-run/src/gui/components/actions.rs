// gui/components/actions.rs
use crate::game_automation::{AutomationCommand, GameState};
use dioxus::prelude::*;
use tokio::sync::mpsc;

#[derive(Props, PartialEq, Clone)]
pub struct ActionsProps {
    pub screenshot_status: Signal<String>,
    pub screenshot_bytes: Signal<Option<Vec<u8>>>,
    pub auto_update_on_touch: Signal<bool>,
    pub select_box: Signal<bool>, // new signal for select box
    pub automation_state: Signal<GameState>,
    pub automation_command_tx: Signal<Option<mpsc::Sender<AutomationCommand>>>,
    pub automation_interval: Signal<u64>,
    pub timed_tap_countdown: Signal<Option<(String, u64)>>, // (id, seconds_remaining)
}

#[component]
pub fn Actions(props: ActionsProps) -> Element {
    let mut screenshot_status = props.screenshot_status;
    let screenshot_bytes = props.screenshot_bytes;
    let mut auto_update_on_touch = props.auto_update_on_touch;
    let mut select_box = props.select_box;
    let automation_state = props.automation_state;
    let automation_command_tx = props.automation_command_tx;
    let mut automation_interval = props.automation_interval;
    let timed_tap_countdown = props.timed_tap_countdown;

    rsx! {
        div { style: "background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 15px; border-radius: 15px; margin-bottom: 15px; border: 1px solid rgba(255,255,255,0.2);",
            // Header
            div { style: "display: flex; align-items: center; justify-content: center; margin-bottom: 15px;",
                h2 { style: "margin: 0; color: #87ceeb; font-size: 1.1em;", "🤖 Automation Controls" }
            }

            // Controls
            div { style: "display: flex; flex-direction: column; gap: 12px;",
                // Status and control buttons
                div { style: "display: flex; gap: 8px; flex-wrap: wrap; align-items: center; justify-content: center;",
                    // Automation state indicator
                    div {
                        style: match *automation_state.read() {
                            GameState::Idle => "background: #666; color: white; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;",
                            GameState::Running => "background: #28a745; color: white; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;",
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
                    if *automation_state.read() == GameState::Running {
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

                // Save and Exit buttons row
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

                // Timed Tap Countdown Display
                if let Some((tap_id, seconds_remaining)) = timed_tap_countdown.read().clone() {
                    div { style: "background: rgba(0,0,0,0.2); border-radius: 8px; padding: 10px 12px; border: 1px solid rgba(255,255,255,0.2);",
                        div { style: "display: flex; align-items: center; justify-content: space-between; gap: 10px;",
                            div { style: "display: flex; flex-direction: column; gap: 2px;",
                                div { style: "display: flex; align-items: center; gap: 6px;",
                                    span { style: "font-size: 0.8em; color: #87ceeb; font-weight: bold;", "🕒 Next Tap:" }
                                    span { style: "font-size: 0.9em; color: #ffd857; font-weight: bold;", "{tap_id}" }
                                }
                                if seconds_remaining > 60 {
                                    span { style: "font-size: 0.75em; color: #ccc; margin-left: 16px;", "in {seconds_remaining / 60}m {seconds_remaining % 60}s" }
                                } else {
                                    span { style: "font-size: 0.75em; color: #ccc; margin-left: 16px;", "in {seconds_remaining}s" }
                                }
                            }
                            div { style: "display: flex; flex-direction: column; align-items: center; gap: 2px;",
                                div { style: "font-family: monospace; font-weight: bold;",
                                    if seconds_remaining < 60 {
                                        span { style: "font-size: 1.1em; color: #ff6b6b;", "{seconds_remaining}s" }
                                    } else if seconds_remaining < 300 {  // Less than 5 minutes
                                        span { style: "font-size: 1.0em; color: #ffd857;", "{seconds_remaining / 60}m" }
                                    } else {
                                        span { style: "font-size: 0.9em; color: #48ff9b;", "{seconds_remaining / 60}m" }
                                    }
                                }
                                // Progress bar showing time remaining
                                div { style: "width: 60px; height: 3px; background: rgba(255,255,255,0.2); border-radius: 2px; overflow: hidden;",
                                    if seconds_remaining <= 10 {
                                        div { style: "height: 100%; background: linear-gradient(to right, #ff4444, #ff8888); border-radius: 2px; animation: pulse 0.5s infinite alternate;",
                                        }
                                    } else if seconds_remaining <= 60 {
                                        div { style: "height: 100%; background: linear-gradient(to right, #ffaa00, #ffdd44); border-radius: 2px;",
                                        }
                                    } else {
                                        div { style: "height: 100%; background: linear-gradient(to right, #28a745, #48ff9b); border-radius: 2px;",
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Screenshot options row
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
            }
        }
    }
}
