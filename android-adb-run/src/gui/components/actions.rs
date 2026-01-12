// gui/components/actions.rs
use crate::game_automation::types::{
    MAX_TAP_INTERVAL_SECONDS, MIN_TAP_INTERVAL_SECONDS, TimedEvent, TimedEventType,
};
use crate::game_automation::{AutomationCommand, GameState};
use crate::gui::dioxus_app::AppContext;
use dioxus::prelude::*;
use tokio::sync::mpsc;

#[component]
pub fn Actions() -> Element {
    let ctx = use_context::<AppContext>();

    // Access grouped signals via the new structure
    let mut screenshot_status = ctx.screenshot.status;
    let screenshot_bytes = ctx.screenshot.bytes;
    
    let mut auto_update_on_touch = ctx.interaction.auto_update_on_touch;
    let mut select_box = ctx.interaction.select_box;
    let hover_tap_preview = ctx.interaction.hover_tap_preview;
    
    let automation_state = ctx.automation.state;
    let automation_command_tx = ctx.automation.command_tx;
    let timed_events_list = ctx.automation.timed_events_list;
    let is_paused_by_touch = ctx.automation.is_paused_by_touch;
    let touch_timeout_remaining = ctx.automation.touch_timeout_remaining;

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
                                    div {
                                        style: "background: rgba(255,255,255,0.05); border-radius: 6px; padding: 8px; margin-bottom: 6px; border: 1px solid rgba(255,255,255,0.1);",
                                        onmouseenter: {
                                            let event_type = event.event_type.clone();
                                            let mut hover_signal = hover_tap_preview;
                                            move |_| {
                                                if let TimedEventType::Tap { x, y } = event_type {
                                                    hover_signal.set(Some((x, y)));
                                                } else {
                                                    hover_signal.set(None);
                                                }
                                            }
                                        },
                                        onmouseleave: {
                                            let mut hover_signal = hover_tap_preview;
                                            move |_| {
                                                hover_signal.set(None);
                                            }
                                        },
                                        div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;",
                                            div { style: "display: flex; align-items: center; gap: 6px;",
                                                span {
                                                    style: "font-size: 0.85em; font-weight: bold;",
                                                    {
                                                        match &event.event_type {
                                                            TimedEventType::Screenshot => "📸".to_string(),
                                                            TimedEventType::Tap { .. } => "👆".to_string(),
                                                            TimedEventType::CountdownUpdate => "⏰".to_string(),
                                                        }
                                                    }
                                                }
                                                span {
                                                    style: "font-size: 0.8em; color: #87ceeb;",
                                                    {event.id.clone()}
                                                }
                                                span {
                                                    style: "font-size: 0.7em; color: #ffd700; background: rgba(255,215,0,0.1); padding: 1px 4px; border-radius: 8px; font-weight: bold;",
                                                    "({event.execution_count})"
                                                }
                                            }

                                            div { style: "display: flex; align-items: center; gap: 4px;",
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

                                        div { style: "display: flex; justify-content: space-between; align-items: center; font-size: 0.75em;",
                                            div {
                                                style: "display: flex; align-items: center; gap: 6px; color: #ccc; flex-wrap: wrap;",
                                                { render_tap_interval_controls(event, automation_command_tx) }
                                                span {
                                                    style: "font-size: 0.75em;",
                                                    {
                                                        let seconds = event.interval.as_secs();
                                                        let label = format_interval_short(seconds);
                                                        format!("Interval: {} ({}s)", label, seconds)
                                                    }
                                                }
                                                if let TimedEventType::Tap { x, y } = &event.event_type {
                                                    span {
                                                        style: "font-size: 0.75em; color: #999;",
                                                        {
                                                            format!("Tap: ({}, {})", x, y)
                                                        }
                                                    }
                                                }
                                            }

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

fn render_tap_interval_controls(
    event: &TimedEvent,
    automation_command_tx: Signal<Option<mpsc::Sender<AutomationCommand>>>,
) -> Element {
    if let TimedEventType::Tap { .. } = &event.event_type {
        let interval_secs = event.interval.as_secs();
        let adjust_step = interval_adjust_step(interval_secs);
        let step_label = format_interval_short(adjust_step);
        let can_increase = interval_secs < MAX_TAP_INTERVAL_SECONDS;
        let can_decrease = interval_secs > MIN_TAP_INTERVAL_SECONDS;
        let increase_delta = adjust_step as i64;
        let decrease_delta = -(adjust_step as i64);
        let event_id_up = event.id.clone();
        let event_id_down = event.id.clone();

        rsx! {
            div { style: "display: flex; gap: 4px; align-items: center;",
                button {
                    style: if can_increase {
                        "background: rgba(255,255,255,0.08); color: #87ceeb; border: 1px solid rgba(135,206,235,0.45); border-radius: 3px; width: 18px; height: 18px; display: flex; align-items: center; justify-content: center; font-size: 0.55em; cursor: pointer; transition: all 0.2s ease;"
                    } else {
                        "background: rgba(255,255,255,0.03); color: #666; border: 1px solid rgba(255,255,255,0.1); border-radius: 3px; width: 18px; height: 18px; display: flex; align-items: center; justify-content: center; font-size: 0.55em; cursor: not-allowed;"
                    },
                    disabled: !can_increase,
                    title: if can_increase {
                        format!("Increase interval by {}", step_label.clone())
                    } else {
                        format!("Maximum interval is {}", format_interval_short(MAX_TAP_INTERVAL_SECONDS))
                    },
                    onclick: {
                        let event_id = event_id_up.clone();
                        let delta = increase_delta;
                        move |_| {
                            if let Some(tx) = automation_command_tx.read().as_ref() {
                                let tx = tx.clone();
                                let event_id = event_id.clone();
                                spawn(async move {
                                    let _ = tx.send(AutomationCommand::AdjustTimedEventInterval {
                                        id: event_id,
                                        delta_seconds: delta,
                                    }).await;
                                });
                            }
                        }
                    },
                    "▲"
                }
                button {
                    style: if can_decrease {
                        "background: rgba(255,255,255,0.08); color: #87ceeb; border: 1px solid rgba(135,206,235,0.45); border-radius: 3px; width: 18px; height: 18px; display: flex; align-items: center; justify-content: center; font-size: 0.55em; cursor: pointer; transition: all 0.2s ease;"
                    } else {
                        "background: rgba(255,255,255,0.03); color: #666; border: 1px solid rgba(255,255,255,0.1); border-radius: 3px; width: 18px; height: 18px; display: flex; align-items: center; justify-content: center; font-size: 0.55em; cursor: not-allowed;"
                    },
                    disabled: !can_decrease,
                    title: if can_decrease {
                        format!("Decrease interval by {}", step_label)
                    } else {
                        format!("Minimum interval is {}", format_interval_short(MIN_TAP_INTERVAL_SECONDS))
                    },
                    onclick: {
                        let event_id = event_id_down.clone();
                        let delta = decrease_delta;
                        move |_| {
                            if let Some(tx) = automation_command_tx.read().as_ref() {
                                let tx = tx.clone();
                                let event_id = event_id.clone();
                                spawn(async move {
                                    let _ = tx.send(AutomationCommand::AdjustTimedEventInterval {
                                        id: event_id,
                                        delta_seconds: delta,
                                    }).await;
                                });
                            }
                        }
                    },
                    "▼"
                }
            }
        }
    } else {
        rsx! {}
    }
}

fn format_interval_short(seconds: u64) -> String {
    if seconds == 0 {
        return "0s".to_string();
    }

    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        if minutes > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}h", hours)
        }
    } else if minutes > 0 {
        if secs > 0 {
            format!("{}m {}s", minutes, secs)
        } else {
            format!("{}m", minutes)
        }
    } else {
        format!("{}s", secs)
    }
}

fn interval_adjust_step(seconds: u64) -> u64 {
    if seconds >= 1800 {
        300 // 5 minutes
    } else if seconds >= 900 {
        120 // 2 minutes
    } else if seconds >= 300 {
        60 // 1 minute
    } else if seconds >= 120 {
        30 // 30 seconds
    } else if seconds >= 60 {
        10 // 10 seconds
    } else {
        5 // 5 seconds for fine control
    }
}
