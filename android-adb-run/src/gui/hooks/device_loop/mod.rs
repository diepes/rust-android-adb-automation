use crate::gui::hooks::types::*;
use dioxus::prelude::*;

mod connection_monitor;
mod device_discovery;
mod initial_screenshot;
mod template_matching_pipeline;

pub use template_matching_pipeline::{decode_screenshot_to_rgb, start_template_matching_phase};

pub fn use_device_loop(
    mut screenshot: ScreenshotSignals,
    mut device: DeviceSignals,
    mut shared_adb_client: SharedAdbClient,
    mut force_update: Signal<u32>,
) {
    use_future(move || async move {
        loop {
            let Some(device_name) =
                device_discovery::discover_device_name(&mut device.status).await
            else {
                continue;
            };

            match device_discovery::connect_device(
                &device_name,
                &mut device,
                &mut force_update,
                &mut shared_adb_client,
            )
            .await
            {
                Ok(shared_client) => {
                    initial_screenshot::spawn_initial_screenshot_task(screenshot, shared_client);

                    connection_monitor::wait_for_disconnection(shared_adb_client, device.status)
                        .await;
                }
                Err(e) => {
                    device_discovery::handle_connection_error(
                        &e,
                        &mut device.status,
                        &mut screenshot.status,
                    )
                    .await;
                }
            }
        }
    });
}
