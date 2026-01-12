use crate::game_automation::types::AutomationSignals;
use crate::game_automation::{AutomationCommand, GameAutomation};
use crate::gui::hooks::types::*;
use dioxus::prelude::*;

/// Initializes game automation loop
/// Uses grouped signal structs for cleaner function signature (5 params vs 14)
pub fn use_automation_loop(
    debug_mode: bool,
    screenshot: ScreenshotSignals,
    device: DeviceSignals,
    mut automation: AutomationStateSignals,
    shared_adb_client: SharedAdbClient,
) {
    use_future(move || async move {
        // Create command channel only (no event channel needed)
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(32);
        automation.command_tx.set(Some(cmd_tx.clone()));

        // Create GameAutomation with signal bundle (maps to backend's AutomationSignals)
        let signals = AutomationSignals {
            screenshot_data: screenshot.data,
            screenshot_bytes: screenshot.bytes,
            screenshot_status: screenshot.status,
            automation_state: automation.state,
            is_paused_by_touch: automation.is_paused_by_touch,
            touch_timeout_remaining: automation.touch_timeout_remaining,
            timed_tap_countdown: automation.timed_tap_countdown,
            timed_events_list: automation.timed_events_list,
            device_info: device.info,
            status: device.status,
            screenshot_counter: screenshot.counter,
        };
        let mut game_automation = GameAutomation::new(cmd_rx, debug_mode, signals);

        // Wait for shared client to be available
        let shared_client = loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if let Some(client) = shared_adb_client.read().clone() {
                break client;
            }
        };

        if let Err(e) = game_automation.set_shared_adb_client(shared_client).await {
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
        let _automation_task = spawn(async move { game_automation.run().await });
    });
}
