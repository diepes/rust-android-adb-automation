use super::*;

impl GameAutomation {
    pub(super) async fn process_command(&mut self, command: AutomationCommand) {
        debug_print!(
            self.debug_enabled,
            "🤖 Processing automation command: {:?}",
            command
        );
        match command {
            AutomationCommand::Start => {
                debug_print!(
                    self.debug_enabled,
                    "🤖 Start command received. Current is_running: {}",
                    self.is_running
                );
                if !self.is_running {
                    if self.adb_client.is_none() {
                        println!("⚠️ Cannot start automation: ADB client not initialized");
                        return;
                    }

                    self.is_running = true;
                    self.change_state(GameState::Running).await;
                    println!(
                        "🚀 Game automation STARTED. is_running={}, state={:?}",
                        self.is_running, self.state
                    );

                    debug_print!(
                        self.debug_enabled,
                        "🚀 Game automation started. Timed events: {} configured",
                        self.timed_events.len()
                    );

                    if self.debug_enabled {
                        for (id, event) in &self.timed_events {
                            let next_in = event
                                .time_until_next()
                                .map(|d| format!("{:.1}s", d.as_secs_f32()))
                                .unwrap_or_else(|| "disabled".to_string());
                            debug_print!(
                                self.debug_enabled,
                                "  🕒 Timed event '{}': {:?} every {}s, next in {}",
                                id,
                                event.event_type,
                                event.interval.as_secs(),
                                next_in
                            );
                        }
                    }

                    self.send_timed_events_list().await;
                    self.send_timed_tap_countdowns().await;
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "🤖 Automation already running, ignoring start command"
                    );
                }
            }
            AutomationCommand::Pause => {
                if self.is_running {
                    self.change_state(GameState::Paused).await;
                    debug_print!(self.debug_enabled, "⏸️ Game automation paused");
                }
            }
            AutomationCommand::Resume => {
                if self.is_running && self.state == GameState::Paused {
                    self.change_state(GameState::Running).await;
                    debug_print!(self.debug_enabled, "▶️ Game automation resumed");
                    self.send_timed_events_list().await;
                    self.send_timed_tap_countdowns().await;
                }
            }
            AutomationCommand::Stop => {
                self.is_running = false;

                if let Some(client_arc) = &self.adb_client {
                    let client_guard = client_arc.lock().await;
                    if let Err(e) = client_guard.stop_touch_monitoring().await {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ Failed to stop touch monitoring: {}",
                            e
                        );
                    } else {
                        debug_print!(self.debug_enabled, "👆 Touch monitoring stopped");
                    }
                }

                self.change_state(GameState::Idle).await;
                debug_print!(self.debug_enabled, "⏹️ Game automation stopped");
            }
            AutomationCommand::ClearTouchActivity => {
                if let Some(client_arc) = &self.adb_client {
                    let client_guard = client_arc.lock().await;
                    if let Err(e) = client_guard.clear_touch_activity().await {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ Failed to clear touch activity: {}",
                            e
                        );
                    } else {
                        debug_print!(
                            self.debug_enabled,
                            "👆 Touch activity cleared - automation resuming"
                        );
                        *self.is_paused_by_touch.write_unchecked() = false;
                        *self.touch_timeout_remaining.write_unchecked() = None;
                    }
                }
            }
            AutomationCommand::RegisterTouchActivity => {
                if let Some(client_arc) = &self.adb_client {
                    let client_guard = client_arc.lock().await;
                    if let Err(e) = client_guard.register_touch_activity().await {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ Failed to register touch activity: {}",
                            e
                        );
                    } else {
                        debug_print!(
                            self.debug_enabled,
                            "👆 GUI touch registered - pausing automation for 30s"
                        );
                        *self.is_paused_by_touch.write_unchecked() = true;
                        *self.touch_timeout_remaining.write_unchecked() = Some(30);
                    }
                }
            }
            AutomationCommand::TakeScreenshot => {
                if let Err(e) = self.take_screenshot().await {
                    debug_print!(self.debug_enabled, "❌ Manual screenshot failed: {}", e);
                }
            }
            AutomationCommand::TestImageRecognition => {
                debug_print!(
                    self.debug_enabled,
                    "🧪 Manual image recognition test requested"
                );
                if let Err(e) = self.test_image_recognition().await {
                    *self.screenshot_status.write_unchecked() = format!("❌ {}", e);
                }
            }
            AutomationCommand::RescanTemplates => {
                debug_print!(self.debug_enabled, "🔄 Template rescan requested");
                if let Err(e) = self.rescan_templates().await {
                    *self.screenshot_status.write_unchecked() = format!("❌ {}", e);
                }
            }
            AutomationCommand::AddTimedEvent(event) => {
                debug_print!(
                    self.debug_enabled,
                    "➕ Adding timed event '{}': {:?} every {}s",
                    event.id,
                    event.event_type,
                    event.interval.as_secs()
                );
                self.timed_events.insert(event.id.clone(), event);
            }
            AutomationCommand::RemoveTimedEvent(id) => {
                if self.timed_events.remove(&id).is_some() {
                    debug_print!(self.debug_enabled, "➖ Removed timed event '{}'", id);
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "⚠️ Timed event '{}' not found for removal",
                        id
                    );
                }
            }
            AutomationCommand::EnableTimedEvent(id) => {
                if let Some(event) = self.timed_events.get_mut(&id) {
                    event.enabled = true;
                    debug_print!(self.debug_enabled, "✅ Enabled timed event '{}'", id);
                    self.send_timed_events_list().await;
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "⚠️ Timed event '{}' not found for enabling",
                        id
                    );
                }
            }
            AutomationCommand::DisableTimedEvent(id) => {
                if let Some(event) = self.timed_events.get_mut(&id) {
                    event.enabled = false;
                    debug_print!(self.debug_enabled, "❌ Disabled timed event '{}'", id);
                    self.send_timed_events_list().await;
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "⚠️ Timed event '{}' not found for disabling",
                        id
                    );
                }
            }
            AutomationCommand::AdjustTimedEventInterval { id, delta_seconds } => {
                if let Some(event) = self.timed_events.get_mut(&id) {
                    if matches!(event.event_type, TimedEventType::Tap { .. }) {
                        let current_secs = event.interval.as_secs();
                        let current_secs_i64 = current_secs as i64;
                        let min_secs = MIN_TAP_INTERVAL_SECONDS as i64;
                        let max_secs = MAX_TAP_INTERVAL_SECONDS as i64;

                        let target_secs = current_secs_i64.saturating_add(delta_seconds);
                        let clamped_secs = target_secs.clamp(min_secs, max_secs) as u64;

                        if clamped_secs != current_secs {
                            event.set_interval(Duration::from_secs(clamped_secs));
                            debug_print!(
                                self.debug_enabled,
                                "⏱️ Adjusted timed event '{}' interval to {}s",
                                id,
                                clamped_secs
                            );
                            self.send_timed_events_list().await;
                            self.send_timed_tap_countdowns().await;
                        } else {
                            debug_print!(
                                self.debug_enabled,
                                "ℹ️ Timed event '{}' interval unchanged ({}s)",
                                id,
                                clamped_secs
                            );
                        }
                    } else {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ Interval adjustments only supported for tap events ({}).",
                            id
                        );
                    }
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "⚠️ Timed event '{}' not found for interval adjustment",
                        id
                    );
                }
            }
            AutomationCommand::TriggerTimedEvent(id) => {
                if let Some(event) = self.timed_events.get(&id) {
                    if event.enabled {
                        debug_print!(
                            self.debug_enabled,
                            "🔫 Triggering timed event '{}' immediately",
                            id
                        );
                        match event.event_type {
                            TimedEventType::Screenshot => {
                                let _ = self.take_screenshot().await;
                            }
                            TimedEventType::Tap { x, y } => {
                                if let Some(adb_client) = &self.adb_client {
                                    let client = adb_client.lock().await;
                                    if let Err(e) = client.tap(x, y).await {
                                        debug_print!(
                                            self.debug_enabled,
                                            "⚠️ Failed to execute tap ({}, {}): {}",
                                            x,
                                            y,
                                            e
                                        );
                                        if is_disconnect_error(&e.to_string()) {
                                            debug_print!(
                                                self.debug_enabled,
                                                "🔌 Device disconnect detected during manual tap trigger: {}",
                                                e
                                            );
                                            self.device_disconnected = true;
                                            self.last_reconnect_attempt = None;
                                            *self.device_info.write_unchecked() = None;
                                            *self.screenshot_data.write_unchecked() = None;
                                            *self.screenshot_bytes.write_unchecked() = None;
                                            *self.screenshot_status.write_unchecked() = format!(
                                                "🔌 USB DISCONNECTED: {} - Please reconnect",
                                                e
                                            );
                                            *self.status.write_unchecked() =
                                                "🔌 Device Disconnected - Paused".to_string();
                                        }
                                    }
                                }
                            }
                            TimedEventType::CountdownUpdate => {
                                debug_print!(
                                    self.debug_enabled,
                                    "⚠️ Cannot manually trigger countdown update event"
                                );
                            }
                        }
                        if let Some(event) = self.timed_events.get_mut(&id) {
                            event.mark_executed();
                        }
                        self.send_timed_events_list().await;
                    } else {
                        debug_print!(
                            self.debug_enabled,
                            "⚠️ Cannot trigger disabled event '{}'",
                            id
                        );
                    }
                } else {
                    debug_print!(
                        self.debug_enabled,
                        "⚠️ Timed event '{}' not found for triggering",
                        id
                    );
                }
            }
            AutomationCommand::ListTimedEvents => {
                let events: Vec<TimedEvent> = self.timed_events.values().cloned().collect();
                debug_print!(
                    self.debug_enabled,
                    "📋 Listing {} timed events",
                    events.len()
                );
                for event in &events {
                    let status = if event.enabled { "enabled" } else { "disabled" };
                    let next_time = match event.time_until_next() {
                        Some(duration) => format!("{:.1}s", duration.as_secs_f32()),
                        None => "disabled".to_string(),
                    };
                    debug_print!(
                        self.debug_enabled,
                        "  - {}: {:?} every {}s, {}, next in {}",
                        event.id,
                        event.event_type,
                        event.interval.as_secs(),
                        status,
                        next_time
                    );
                }
                *self.timed_events_list.write_unchecked() = events;
            }
            AutomationCommand::Shutdown => {
                self.should_exit = true;
                self.is_running = false;
                self.change_state(GameState::Idle).await;
                println!("🛑 Game automation shutting down");
            }
        }
    }
}
