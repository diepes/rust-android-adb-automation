use super::*;

impl GameAutomation {
    pub(super) async fn check_reconnection(&mut self) {
        let backoff_secs = match self.last_reconnect_attempt {
            None => 0,
            Some(last_attempt) => {
                let elapsed = std::time::Instant::now()
                    .duration_since(last_attempt)
                    .as_secs();
                let attempt_count = (elapsed / 10).min(4);
                let backoff = 2u64.pow(attempt_count as u32);
                backoff.min(30)
            }
        };

        let now = std::time::Instant::now();
        let should_attempt = match self.last_reconnect_attempt {
            None => true,
            Some(last_attempt) => {
                let elapsed = now.duration_since(last_attempt).as_secs();
                elapsed >= backoff_secs
            }
        };

        if should_attempt {
            println!(
                "🔄 Attempting device reconnection (elapsed: {:?})...",
                self.last_reconnect_attempt
                    .map(|t| now.duration_since(t))
                    .unwrap_or_default()
            );
            self.last_reconnect_attempt = Some(now);

            if let Ok(()) = self.attempt_reconnection().await {
                return;
            }
        }

        if let Some(last_attempt) = self.last_reconnect_attempt {
            let elapsed = now.duration_since(last_attempt).as_secs();
            let backoff_secs = match self.last_reconnect_attempt {
                None => 0,
                Some(last_attempt) => {
                    let elapsed = std::time::Instant::now()
                        .duration_since(last_attempt)
                        .as_secs();
                    let attempt_count = (elapsed / 10).min(4);
                    let backoff = 2u64.pow(attempt_count as u32);
                    backoff.min(30)
                }
            };
            let remaining = backoff_secs.saturating_sub(elapsed);

            if remaining > 0 {
                *self.screenshot_status.write_unchecked() =
                    format!("🔌 Device disconnected - Next retry in {}s...", remaining);
            } else {
                *self.screenshot_status.write_unchecked() =
                    "🔌 Device disconnected - Attempting reconnection...".to_string();
            }
        }
    }

    pub(super) async fn attempt_reconnection(&mut self) -> Result<(), String> {
        println!("🔄 Attempting to reconnect to device...");

        if let Some(old_client_arc) = self.adb_client.take() {
            println!("🔧 Shutting down old USB connection...");
            match Arc::try_unwrap(old_client_arc) {
                Ok(mutex) => {
                    let mut old_client = mutex.into_inner();
                    match old_client.shutdown().await {
                        Ok(_) => {
                            println!("✅ Old connection shut down cleanly");
                        }
                        Err(e) => {
                            println!("⚠️ Old connection shutdown warning: {}", e);
                        }
                    }
                }
                Err(arc) => {
                    println!("⚠️ Old connection has other references, forcing drop...");
                    drop(arc);
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        match AdbBackend::connect_first().await {
            Ok(client) => {
                let (screen_width, screen_height) = client.screen_dimensions();
                println!(
                    "✅ Device reconnected! ({}x{})",
                    screen_width, screen_height
                );

                let mut config = create_default_config();
                config.debug_enabled = self.debug_enabled;
                self.game_detector = GameStateDetector::new(screen_width, screen_height, config);

                self.adb_client = Some(Arc::new(Mutex::new(client)));

                if let Some(client_arc) = &self.adb_client {
                    let client_guard = client_arc.lock().await;
                    if let Err(e) = client_guard.start_touch_monitoring().await {
                        println!("⚠️ Failed to start touch monitoring after reconnect: {}", e);
                    } else {
                        println!("👆 Touch monitoring restarted");
                    }
                }

                self.device_disconnected = false;
                self.last_reconnect_attempt = None;

                if self.is_running && self.state == GameState::Paused {
                    self.change_state(GameState::Running).await;
                    println!("▶️ Auto-resuming automation after reconnection");
                }

                if let Some(client_arc) = &self.adb_client {
                    let client_guard = client_arc.lock().await;
                    let (sx, sy) = client_guard.screen_dimensions();
                    *self.device_info.write_unchecked() = Some(DeviceInfo {
                        name: client_guard.device_name().to_string(),
                        transport_id: client_guard.transport_id(),
                        screen_x: sx,
                        screen_y: sy,
                    });
                }
                *self.screenshot_status.write_unchecked() =
                    "✅ Reconnected! Automation ready.".to_string();
                *self.status.write_unchecked() = "✅ Device Reconnected - Resuming".to_string();

                println!("✅ Device reconnected successfully - automation auto-resumed");

                Ok(())
            }
            Err(e) => {
                println!("❌ Reconnection failed: {}", e);
                Err(e.to_string())
            }
        }
    }
}
