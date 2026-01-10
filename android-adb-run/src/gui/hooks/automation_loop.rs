use crate::game_automation::types::AutomationSignals;
use crate::game_automation::{AutomationCommand, GameAutomation, GameState};
use crate::gui::hooks::types::*;
use dioxus::prelude::*;

/// Initializes game automation loop
pub fn use_automation_loop(
    debug_mode: bool,
    mut automation_command_tx: CommandTxSignal,
    automation_state: Signal<GameState>,
    screenshot_counter: Signal<u64>,
    screenshot_data: ScreenshotDataSignal,
    screenshot_bytes: ScreenshotBytesSignal,
    screenshot_status: Signal<String>,
    timed_tap_countdown: TimedCountdownSignal,
    timed_events_list: TimedEventsSignal,
    is_paused_by_touch: Signal<bool>,
    touch_timeout_remaining: TouchTimeoutSignal,
    device_info: DeviceInfoSignal,
    status: Signal<String>,
    shared_adb_client: SharedAdbClient,
) {
    use_future(move || async move {
        // Create command channel only (no event channel needed)
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(32);
        automation_command_tx.set(Some(cmd_tx.clone()));

        // Create GameAutomation with signal bundle
        let signals = AutomationSignals {
            screenshot_data,
            screenshot_bytes,
            screenshot_status,
            automation_state,
            is_paused_by_touch,
            touch_timeout_remaining,
            timed_tap_countdown,
            timed_events_list,
            device_info,
            status,
            screenshot_counter,
        };
        let mut automation = GameAutomation::new(cmd_rx, debug_mode, signals);

        // Wait for shared client to be available
        let shared_client = loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if let Some(client) = shared_adb_client.read().clone() {
                break client;
            }
        };

        if let Err(e) = automation.set_shared_adb_client(shared_client).await {
            log::error!("Failed to set shared automation ADB client: {}", e);
            return; // Don't start automation if client setup fails
        }

        // Auto-start automation BEFORE spawning run task
        let auto_start_tx = cmd_tx.clone();
        spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let _ = auto_start_tx.send(AutomationCommand::Start).await;
        });

        // Start automation run loop in background (AFTER client is set)
        let _automation_task = spawn(async move { automation.run().await });
    });
}
