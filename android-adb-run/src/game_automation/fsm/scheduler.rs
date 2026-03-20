use super::*;

impl GameAutomation {
    pub(super) async fn process_timed_events(&mut self) {
        if let Some(client) = &self.adb_client {
            let (human_touching, remaining_seconds) = {
                let client_guard = client.lock().await;
                let touching = client_guard.is_human_touching().await;
                let remaining = if touching {
                    client_guard.get_touch_timeout_remaining().await
                } else {
                    None
                };
                (touching, remaining)
            };

            if human_touching {
                debug_print!(
                    self.debug_enabled,
                    "🚫 AUTOMATION PAUSED: Human touch detected - skipping timed events"
                );
                *self.is_paused_by_touch.write_unchecked() = true;
                *self.touch_timeout_remaining.write_unchecked() = remaining_seconds;
                return;
            } else {
                use std::sync::LazyLock;
                use std::sync::Mutex as StdMutex;

                static LAST_NO_ACTIVITY_SENT: LazyLock<StdMutex<std::time::Instant>> =
                    LazyLock::new(|| StdMutex::new(std::time::Instant::now()));

                let should_send = {
                    let mut last_sent = LAST_NO_ACTIVITY_SENT.lock().unwrap();
                    let elapsed = last_sent.elapsed().as_secs();
                    if elapsed > 5 {
                        *last_sent = std::time::Instant::now();
                        true
                    } else {
                        false
                    }
                };

                if should_send {
                    debug_print!(
                        self.debug_enabled,
                        "✅ AUTOMATION ACTIVE: No human touch detected - processing events"
                    );
                    *self.is_paused_by_touch.write_unchecked() = false;
                    *self.touch_timeout_remaining.write_unchecked() = None;
                }
            }
        }

        let mut events_to_execute = Vec::new();
        for (id, event) in &self.timed_events {
            if event.is_ready(self.debug_enabled) {
                debug_print!(self.debug_enabled, "✓ Event '{}' is READY", id);
                events_to_execute.push((id.clone(), event.event_type.clone()));
            }
        }

        events_to_execute.sort_by(|a, b| {
            let order_a = match a.1 {
                TimedEventType::Screenshot => 0,
                TimedEventType::CountdownUpdate => 1,
                TimedEventType::Tap { .. } => 2,
            };
            let order_b = match b.1 {
                TimedEventType::Screenshot => 0,
                TimedEventType::CountdownUpdate => 1,
                TimedEventType::Tap { .. } => 2,
            };
            order_a.cmp(&order_b)
        });

        if events_to_execute.is_empty() {
            static EMPTY_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let count = EMPTY_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count.is_multiple_of(30) && count > 0 {
                println!("📭 No events ready (checked {} times)", count);
                for (id, event) in &self.timed_events {
                    if let Some(last) = event.last_executed {
                        let elapsed = last.elapsed();
                        println!(
                            "  - {}: elapsed={:?} vs interval={:?}",
                            id, elapsed, event.interval
                        );
                    } else {
                        println!("  - {}: never executed", id);
                    }
                }
            }
        }

        for (event_id, event_type) in events_to_execute {
            if let Err(e) = self.execute_timed_event(&event_id, &event_type).await {
                debug_print!(
                    self.debug_enabled,
                    "❌ Timed event '{}' failed: {}",
                    event_id,
                    e
                );

                if is_disconnect_error(&e.to_string()) {
                    debug_print!(
                        self.debug_enabled,
                        "🔌 Device disconnect detected during timed event: {}",
                        e
                    );

                    self.change_state(GameState::Paused).await;
                    self.device_disconnected = true;
                    self.last_reconnect_attempt = None;

                    *self.device_info.write_unchecked() = None;
                    *self.screenshot_data.write_unchecked() = None;
                    *self.screenshot_bytes.write_unchecked() = None;
                    *self.screenshot_status.write_unchecked() =
                        format!("🔌 USB DISCONNECTED: {} - Please reconnect", e);
                    *self.status.write_unchecked() = "🔌 Device Disconnected - Paused".to_string();
                    return;
                } else {
                    *self.screenshot_status.write_unchecked() =
                        format!("❌ Timed event '{}' failed: {}", event_id, e);
                }
            }
        }
    }

    pub(super) async fn execute_timed_event(
        &mut self,
        event_id: &str,
        event_type: &TimedEventType,
    ) -> Result<(), String> {
        debug_print!(
            self.debug_enabled,
            "⚡ Executing timed event '{}': {:?}",
            event_id,
            event_type
        );

        match event_type {
            TimedEventType::Screenshot => {
                if let Some(client) = &self.adb_client {
                    let client_clone = client.clone();
                    let screenshot_data = self.screenshot_data;
                    let screenshot_bytes_sig = self.screenshot_bytes;
                    let screenshot_status = self.screenshot_status;
                    let mut screenshot_counter = self.screenshot_counter;

                    dioxus::prelude::spawn(async move {
                        let start = std::time::Instant::now();
                        match timeout(Duration::from_secs(10), async {
                            let guard = client_clone.lock().await;
                            guard.screen_capture_bytes().await
                        })
                        .await
                        {
                            Ok(Ok(bytes)) => {
                                let duration_ms = start.elapsed().as_millis();
                                let counter_val = screenshot_counter.with_mut(|c| {
                                    *c += 1;
                                    *c
                                });

                                let bytes_clone = bytes.clone();
                                dioxus::prelude::spawn(async move {
                                    use crate::gui::util::base64_encode;
                                    let base64_string = tokio::task::spawn_blocking(move || {
                                        base64_encode(&bytes_clone)
                                    })
                                    .await
                                    .unwrap_or_default();
                                    *screenshot_data.write_unchecked() = Some(base64_string);
                                    *screenshot_bytes_sig.write_unchecked() = Some(bytes);
                                    *screenshot_status.write_unchecked() = format!(
                                        "🤖 Automation screenshot #{} ({}ms)",
                                        counter_val, duration_ms
                                    );
                                });
                            }
                            Ok(Err(e)) => {
                                *screenshot_status.write_unchecked() =
                                    format!("❌ Screenshot failed: {}", e);
                            }
                            Err(_) => {
                                *screenshot_status.write_unchecked() =
                                    "❌ Screenshot timeout (10s)".to_string();
                            }
                        }
                    });
                }
            }
            TimedEventType::Tap { x, y } => {
                if let Some(client) = &self.adb_client {
                    debug_print!(
                        self.debug_enabled,
                        "🎯 Queuing tap: {} at ({},{})",
                        event_id,
                        x,
                        y
                    );
                    let result = {
                        let client_guard = client.lock().await;
                        client_guard.tap(*x, *y).await
                    };

                    match result {
                        Ok(()) => {
                            debug_print!(self.debug_enabled, "✅ {} queued", event_id);
                        }
                        Err(e) => {
                            let error_str = e.to_string();
                            println!("❌ {} queue failed: {}", event_id, error_str);

                            if is_disconnect_error(&error_str) {
                                debug_print!(
                                    self.debug_enabled,
                                    "🔌 Device disconnect detected during tap '{}': {}",
                                    event_id,
                                    error_str
                                );
                                self.device_disconnected = true;
                                self.last_reconnect_attempt = None;
                                *self.device_info.write_unchecked() = None;
                                *self.screenshot_data.write_unchecked() = None;
                                *self.screenshot_bytes.write_unchecked() = None;
                                *self.screenshot_status.write_unchecked() = format!(
                                    "🔌 USB DISCONNECTED: {} (during tap) - Please reconnect",
                                    error_str
                                );
                                *self.status.write_unchecked() =
                                    "🔌 Device Disconnected - Paused".to_string();
                            }
                        }
                    }
                } else {
                    return Err("ADB client not available".to_string());
                }
            }
            TimedEventType::CountdownUpdate => {
                self.send_timed_events_list().await;
                self.send_timed_tap_countdowns().await;

                for (id, event) in &self.timed_events {
                    if id.contains("tap")
                        && let Some(last) = event.last_executed
                    {
                        let elapsed = last.elapsed();
                        let remaining = if elapsed < event.interval {
                            event.interval - elapsed
                        } else {
                            Duration::from_secs(0)
                        };
                        debug_print!(
                            self.debug_enabled,
                            "⏰ {}: {}s remaining",
                            id,
                            remaining.as_secs()
                        );
                    }
                }
            }
        }

        if let Some(event) = self.timed_events.get_mut(event_id) {
            event.mark_executed();
        }

        Ok(())
    }

    pub(super) async fn send_timed_tap_countdowns(&self) {
        if let Some((next_tap_id, seconds_remaining)) = self.get_next_tap_info() {
            *self.timed_tap_countdown.write_unchecked() = Some((next_tap_id, seconds_remaining));
        }
    }

    pub(super) async fn send_timed_events_list(&self) {
        let events: Vec<crate::game_automation::types::TimedEvent> =
            self.timed_events.values().cloned().collect();
        *self.timed_events_list.write_unchecked() = events;
    }

    fn get_next_tap_info(&self) -> Option<(String, u64)> {
        let mut next_tap: Option<(String, u64)> = None;

        for (id, event) in &self.timed_events {
            if !matches!(event.event_type, TimedEventType::Tap { .. }) {
                continue;
            }

            if !event.enabled {
                continue;
            }

            if let Some(time_until_next) = event.time_until_next() {
                let seconds_remaining = time_until_next.as_secs();

                match &next_tap {
                    None => {
                        next_tap = Some((id.clone(), seconds_remaining));
                    }
                    Some((_, current_min_seconds)) => {
                        if seconds_remaining < *current_min_seconds {
                            next_tap = Some((id.clone(), seconds_remaining));
                        }
                    }
                }
            }
        }

        next_tap
    }
}
