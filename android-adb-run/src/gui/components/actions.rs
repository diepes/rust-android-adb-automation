// gui/components/actions.rs
use crate::game_automation::types::TimedEvent;
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
    pub timed_tap_countdown: Signal<Option<(String, u64)>>, // (id, seconds_remaining)
    pub timed_events_list: Signal<Vec<TimedEvent>>,         // All timed events
    pub is_paused_by_touch: Signal<bool>,                   // Touch-based pause indicator
    pub touch_timeout_remaining: Signal<Option<u64>>,       // Remaining seconds until touch timeout expires
}

#[component]
pub fn Actions(props: ActionsProps) -> Element {
    let mut screenshot_status = props.screenshot_status;
    let screenshot_bytes = props.screenshot_bytes;
    let mut auto_update_on_touch = props.auto_update_on_touch;
    let mut select_box = props.select_box;
    let automation_state = props.automation_state;
    let automation_command_tx = props.automation_command_tx;
    let timed_events_list = props.timed_events_list;
    let is_paused_by_touch = props.is_paused_by_touch;
    let touch_timeout_remaining = props.touch_timeout_remaining;

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
                    // Automation state indicator with touch pause detection
                    {
                        let is_touch_paused = *is_paused_by_touch.read();
                        let state = automation_state.read().clone();
                        let remaining_secs = *touch_timeout_remaining.read();
                        
                        let (display_text, style) = if is_touch_paused && state == GameState::Running {
                            // Show countdown if available
                            let text = if let Some(secs) = remaining_secs {
                                format!("Paused - {} sec", secs)
                            } else {
                                "Paused - Activity".to_string()
                            };
                            (text, "background: #ff6b6b; color: white; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;".to_string())
                        } else {
                            let (text, style_str) = match state {
                                GameState::Idle => ("Idle".to_string(), "background: #666; color: white; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;"),
                                GameState::Running => ("Running".to_string(), "background: #28a745; color: white; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;"),
                                GameState::Paused => ("Paused".to_string(), "background: #fd7e14; color: white; padding: 4px 10px; border-radius: 16px; font-size: 0.8em; font-weight: 600;"),
                            };
                            (text, style_str.to_string())
                        };
                        rsx! {
                            div { style: "{style}", "{display_text}" }
                        }
                    }

                    // Control buttons - show Resume when touch paused
                    {
                        let is_touch_paused = *is_paused_by_touch.read();
                        let state = automation_state.read().clone();
                        let effective_state = if is_touch_paused && state == GameState::Running {
                            GameState::Paused
                        } else {
                            state.clone()
                        };
                        
                        rsx! {
                            if effective_state == GameState::Idle || effective_state == GameState::Paused {
                                button { 
                                    style: "background: linear-gradient(45deg, #28a745, #20c997); color: white; padding: 6px 12px; border: none; border-radius: 6px; cursor: pointer; font-size: 0.85em; font-weight: bold;",
                                    onclick: {
                                        let state_for_click = effective_state.clone();
                                        let actual_state = state.clone();
                                        let touch_paused = is_touch_paused;
                                        move |_| {
                                            if let Some(tx) = automation_command_tx.read().as_ref() {
                                                let tx = tx.clone();
                                                let is_paused = state_for_click == GameState::Paused;
                                                let is_idle = actual_state == GameState::Idle;                                spawn(async move {
                                    // If touch paused but clicking resume, clear the touch activity
                                    if touch_paused {
                                        let _ = tx.send(AutomationCommand::ClearTouchActivity).await;
                                    } else {
                                        let cmd = if is_paused || !is_idle {
                                            AutomationCommand::Resume
                                        } else {
                                            AutomationCommand::Start
                                        };
                                        let _ = tx.send(cmd).await;
                                    }
                                });
                                            }
                                        }
                                    },
                                    // Always show Resume when paused (either manually or by touch)
                                    if effective_state == GameState::Paused { 
                                        "▶️ Resume" 
                                    } else { 
                                        "🚀 Start" 
                                    }
                                }
                            }
                            if effective_state == GameState::Running {
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
                            if effective_state != GameState::Idle {
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

                // Timed Events List Display
                if !timed_events_list.read().is_empty() {
                    div { style: "background: rgba(0,0,0,0.2); border-radius: 8px; padding: 10px 12px; border: 1px solid rgba(255,255,255,0.2);",
                        // Filter out system events and count visible events
                        {
                            let events = timed_events_list.read();
                            let visible_events: Vec<_> = events.iter()
                                .filter(|event| event.id != "countdown_update")
                                .collect();

                            rsx! {
                                div { style: "display: flex; align-items: center; gap: 6px; margin-bottom: 8px;",
                                    span { style: "font-size: 0.9em; color: #87ceeb; font-weight: bold;", "🕒 Timed Events" }
                                    span { style: "font-size: 0.75em; color: #ccc;", "({visible_events.len()} events)" }
                                }

                                // Individual event displays
                                for event in visible_events {
                                    div { style: "background: rgba(255,255,255,0.05); border-radius: 6px; padding: 8px; margin-bottom: 6px; border: 1px solid rgba(255,255,255,0.1);",
                                        div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;",
                                            // Event name and type
                                            div { style: "display: flex; align-items: center; gap: 6px;",
                                                span {
                                                    style: "font-size: 0.85em; font-weight: bold;",
                                                    {
                                                        match &event.event_type {
                                                            crate::game_automation::types::TimedEventType::Screenshot => "📸".to_string(),
                                                            crate::game_automation::types::TimedEventType::Tap { .. } => "👆".to_string(),
                                                            crate::game_automation::types::TimedEventType::CountdownUpdate => "⏰".to_string(),
                                                        }
                                                    }
                                                }
                                                span {
                                                    style: "font-size: 0.8em; color: #87ceeb;",
                                                    {event.id.clone()}
                                                }
                                                // Execution counter
                                                span {
                                                    style: "font-size: 0.7em; color: #ffd700; background: rgba(255,215,0,0.1); padding: 1px 4px; border-radius: 8px; font-weight: bold;",
                                                    "({event.execution_count})"
                                                }
                                            }

                                            // Control buttons row
                                            div { style: "display: flex; align-items: center; gap: 4px;",
                                                // Status indicator (clickable toggle)
                                                button {
                                                    style: if event.enabled {
                                                        "background: #28a745; color: white; padding: 2px 6px; border-radius: 10px; font-size: 0.7em; font-weight: bold; border: none; cursor: pointer; transition: all 0.2s ease;"
                                                    } else {
                                                        "background: #6c757d; color: white; padding: 2px 6px; border-radius: 10px; font-size: 0.7em; font-weight: bold; border: none; cursor: pointer; transition: all 0.2s ease;"
                                                    },
                                                    onclick: {
                                                        let event_id = event.id.clone();
                                                        let is_enabled = event.enabled;
                                                        move |_| {
                                                            if let Some(tx) = automation_command_tx.read().as_ref() {
                                                                let tx = tx.clone();
                                                                let event_id = event_id.clone();
                                                                spawn(async move {
                                                                    let cmd = if is_enabled {
                                                                        AutomationCommand::DisableTimedEvent(event_id)
                                                                    } else {
                                                                        AutomationCommand::EnableTimedEvent(event_id)
                                                                    };
                                                                    let _ = tx.send(cmd).await;
                                                                });
                                                            }
                                                        }
                                                    },
                                                    title: if event.enabled { "Click to disable this event" } else { "Click to enable this event" },
                                                    if event.enabled { "ON" } else { "OFF" }
                                                }

                                                // Fire immediately button
                                                button {
                                                    style: if event.enabled {
                                                        "background: #dc3545; color: white; padding: 2px 6px; border-radius: 10px; font-size: 0.7em; font-weight: bold; border: none; cursor: pointer; transition: all 0.2s ease; min-width: 24px;"
                                                    } else {
                                                        "background: #6c757d; color: #999; padding: 2px 6px; border-radius: 10px; font-size: 0.7em; font-weight: bold; border: none; cursor: not-allowed; transition: all 0.2s ease; min-width: 24px;"
                                                    },
                                                    disabled: !event.enabled,
                                                    onclick: {
                                                        let event_id = event.id.clone();
                                                        let is_enabled = event.enabled;
                                                        move |_| {
                                                            if is_enabled
                                                                && let Some(tx) = automation_command_tx.read().as_ref() {
                                                                    let tx = tx.clone();
                                                                    let event_id = event_id.clone();
                                                                    spawn(async move {
                                                                        let _ = tx.send(AutomationCommand::TriggerTimedEvent(event_id)).await;
                                                                    });
                                                                }
                                                        }
                                                    },
                                                    title: if event.enabled {
                                                        "Click to trigger this event immediately"
                                                    } else {
                                                        "Enable event first to trigger it"
                                                    },
                                                    "🔫"
                                                }
                                            }
                                        }

                                        // Countdown info
                                        div { style: "display: flex; justify-content: space-between; align-items: center; font-size: 0.75em;",
                                            div { style: "color: #ccc;",
                                                "Interval: {event.interval.as_secs()}s"
                                                {
                                                    match &event.event_type {
                                                        crate::game_automation::types::TimedEventType::Tap { x, y } =>
                                                            format!(" | Tap: ({}, {})", x, y),
                                                        _ => String::new()
                                                    }
                                                }
                                            }

                                            // Time remaining display
                                            div { style: "color: #87ceeb; font-weight: bold;",
                                                {
                                                    if let Some(time_until) = event.time_until_next() {
                                                        let seconds = time_until.as_secs();
                                                        if seconds == 0 {
                                                            "Ready Now".to_string()
                                                        } else if seconds < 60 {
                                                            format!("{}s", seconds)
                                                        } else {
                                                            format!("{}m {}s", seconds / 60, seconds % 60)
                                                        }
                                                    } else {
                                                        "Disabled".to_string()
                                                    }
                                                }
                                            }
                                        }

                                        // Progress bar
                                        if event.enabled {
                                            div { style: "margin-top: 4px; background: rgba(255,255,255,0.1); border-radius: 3px; height: 4px; overflow: hidden;",
                                                div {
                                                    style: {
                                                        
                                                        if let Some(time_until) = event.time_until_next() {
                                                            let total_seconds = event.interval.as_secs() as f64;
                                                            let remaining_seconds = time_until.as_secs() as f64;
                                                            let progress = ((total_seconds - remaining_seconds) / total_seconds * 100.0).clamp(0.0, 100.0);
                                                            format!("background: linear-gradient(90deg, #28a745, #20c997); width: {}%; height: 100%; transition: width 0.5s ease;", progress)
                                                        } else {
                                                            "background: #6c757d; width: 0%; height: 100%;".to_string()
                                                        }
                                                    }
                                                }
                                            }
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
            }
        }
    }
}
