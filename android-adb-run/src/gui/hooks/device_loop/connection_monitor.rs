use crate::adb::AdbClient;
use crate::gui::hooks::types::SharedAdbClient;
use dioxus::prelude::*;

pub(super) async fn wait_for_disconnection(
    monitor_shared_client: SharedAdbClient,
    mut device_status: Signal<String>,
) {
    let mut check_interval = tokio::time::interval(tokio::time::Duration::from_secs(3));

    loop {
        check_interval.tick().await;

        if monitor_shared_client.read().is_none() {
            log::debug!("Device monitoring: Client cleared, device disconnected");
            device_status.set("🔌 Device Disconnected - Searching for device...".to_string());
            break;
        }

        if let Some(client_arc) = monitor_shared_client.read().clone() {
            let client_lock = client_arc.lock().await;
            let _ = client_lock.screen_dimensions();
            drop(client_lock);
        } else {
            break;
        }
    }

    log::debug!("Device monitoring task ending, returning to discovery phase");
}
