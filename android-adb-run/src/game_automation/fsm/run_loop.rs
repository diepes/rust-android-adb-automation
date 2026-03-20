use super::*;

impl GameAutomation {
    pub async fn run(&mut self) {
        debug_print!(self.debug_enabled, "🎮 Event-driven automation FSM started");
        println!("🎮 Automation run() loop starting");

        let mut loop_count = 0u32;
        loop {
            loop_count += 1;
            if loop_count.is_multiple_of(10) {
                debug_print!(
                    self.debug_enabled,
                    "💓 Loop alive: {}, is_running={}",
                    loop_count,
                    self.is_running
                );
            }

            match timeout(Duration::from_secs(1), self.command_rx.recv()).await {
                Ok(Some(command)) => {
                    self.process_command(command).await;
                }
                Ok(None) => {
                    debug_print!(self.debug_enabled, "🔌 Command channel closed");
                    break;
                }
                Err(_) => {}
            }

            if self.device_disconnected {
                self.check_reconnection().await;
            }

            if self.is_running && self.state != GameState::Paused {
                self.process_timed_events().await;
            } else {
                static ONCE: std::sync::Once = std::sync::Once::new();
                ONCE.call_once(|| {
                    println!(
                        "⚠️ NOT processing events: is_running={}, state={:?}",
                        self.is_running, self.state
                    );
                });
            }

            if self.should_exit {
                break;
            }
        }

        debug_print!(self.debug_enabled, "🎮 Event-driven automation FSM ended");
    }
}
