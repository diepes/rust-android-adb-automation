use crate::adb::{AdbBackend, AdbClient};
use crate::gui::hooks::types::*;
use dioxus::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

pub(super) type ErrorConfig = (Box<dyn Fn(&String) -> String>, &'static str, u32);

pub(super) async fn discover_device_name(device_status: &mut Signal<String>) -> Option<String> {
    device_status.set("🔍 Looking for devices...".to_string());
    let devices = match AdbBackend::list_devices().await {
        Ok(devices) if !devices.is_empty() => devices,
        Ok(_) => {
            for seconds in (1..=5).rev() {
                device_status.set(format!(
                    "🔌 No Device Connected - Retrying in {}s...",
                    seconds
                ));
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
            return None;
        }
        Err(e) => {
            for seconds in (1..=5).rev() {
                device_status.set(format!("❌ Error: {} - Retrying in {}s...", e, seconds));
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
            return None;
        }
    };

    let first_device = &devices[0];
    device_status.set(format!("📱 Found device: {}", first_device.name));
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    device_status.set(format!("🔌 Connecting to {}...", first_device.name));

    Some(first_device.name.clone())
}

pub(super) async fn connect_device(
    device_name: &str,
    device: &mut DeviceSignals,
    force_update: &mut Signal<u32>,
    shared_adb_client: &mut SharedAdbClient,
) -> Result<Arc<TokioMutex<AdbBackend>>, crate::adb::AdbError> {
    let client = AdbBackend::new_with_device(device_name).await?;

    let (sx, sy) = client.screen_dimensions();
    device.info.set(Some(DeviceInfo {
        name: client.device_name().to_string(),
        transport_id: client.transport_id(),
        screen_x: sx,
        screen_y: sy,
    }));
    device.status.set("✅ Connected".to_string());
    force_update.with_mut(|v| *v = v.wrapping_add(1));

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let shared_client = Arc::new(TokioMutex::new(client));
    shared_adb_client.set(Some(shared_client.clone()));

    Ok(shared_client)
}

pub(super) async fn handle_connection_error(
    error: &crate::adb::AdbError,
    device_status: &mut Signal<String>,
    screenshot_status: &mut Signal<String>,
) {
    let (get_status, tip_msg, retry_secs): ErrorConfig = if error.is_resource_busy() {
        (
            Box::new(|_e| {
                "⚠️ USB Already in Use - Close other ADB apps - Retrying in {}s...".to_string()
            }),
            "💡 Close other instances (VS Code, Android Studio, etc.)",
            10u32,
        )
    } else if error.is_permission_denied() {
        (
            Box::new(|_e| {
                "⚠️ Permission Denied - Check USB permissions - Retrying in {}s...".to_string()
            }),
            "💡 Run: sudo chmod 666 /dev/bus/usb/*/0*",
            5u32,
        )
    } else if error.is_device_not_found() {
        (
            Box::new(|_e| {
                "⚠️ No Device Found - Reconnect USB cable - Retrying in {}s...".to_string()
            }),
            "💡 Unplug and replug the USB cable",
            5u32,
        )
    } else {
        (
            Box::new(|e: &String| format!("❌ Connection failed: {} - Retrying in {{}}s...", e)),
            "⏳ Waiting for USB authorization...",
            5u32,
        )
    };

    for seconds in (1..=retry_secs).rev() {
        let msg = get_status(&error.to_string());
        device_status.set(msg.replace("{}", &seconds.to_string()));
        screenshot_status.set(tip_msg.to_string());
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
