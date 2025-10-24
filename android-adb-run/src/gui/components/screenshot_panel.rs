// gui/components/screenshot_panel.rs
use crate::adb::Adb;
use crate::gui::util::base64_encode;
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct ScreenshotPanelProps {
    pub screenshot_status: Signal<String>,
    pub screenshot_data: Signal<Option<String>>,
    pub screenshot_bytes: Signal<Option<Vec<u8>>>,
    pub device_info: Signal<Option<(String, u32, u32, u32)>>,
    pub device_coords: Signal<Option<(u32, u32)>>,
    pub mouse_coords: Signal<Option<(i32, i32)>>,
    pub is_loading_screenshot: Signal<bool>,
    pub auto_update_on_touch: Signal<bool>,
    pub is_swiping: Signal<bool>,
    pub swipe_start: Signal<Option<(u32, u32)>>,
    pub swipe_end: Signal<Option<(u32, u32)>>,
    pub calculate_device_coords: fn(dioxus::html::geometry::ElementPoint, u32, u32) -> (u32, u32),
}

#[component]
pub fn ScreenshotPanel(props: ScreenshotPanelProps) -> Element {
    let loading = *props.is_loading_screenshot.read();
    let mut screenshot_status = props.screenshot_status;
    let mut screenshot_data = props.screenshot_data;
    let mut screenshot_bytes = props.screenshot_bytes;
    let device_info = props.device_info; // read-only
    let mut device_coords = props.device_coords;
    let mut mouse_coords = props.mouse_coords;
    let mut is_swiping = props.is_swiping;
    let mut swipe_start = props.swipe_start;
    let mut swipe_end = props.swipe_end;
    let auto_update_on_touch = props.auto_update_on_touch;
    let mut is_loading_screenshot = props.is_loading_screenshot;
    let calculate_device_coords = props.calculate_device_coords;
    let status_text = screenshot_status.read().clone();
    rsx! {
        div { style: "flex:0 0 400px; background:rgba(255,255,255,0.1); backdrop-filter:blur(10px); padding:15px; border-radius:15px; border:1px solid rgba(255,255,255,0.2); height:fit-content;",
            if let Some(image_data) = screenshot_data.read().as_ref() {
                div { style: "text-align:center; position:relative;",
                    img { src: "data:image/png;base64,{image_data}",
                        style: if loading { "max-width:100%; max-height:600px; border-radius:10px; cursor:crosshair; border:8px solid #ff4444; box-shadow:0 0 40px rgba(255,68,68,0.8); user-select:none;" }
                                else { "max-width:100%; max-height:600px; border-radius:10px; cursor:crosshair; border:8px solid rgba(255,255,255,0.2); box-shadow:0 4px 15px rgba(0,0,0,0.3); user-select:none;" },
                        onmousemove: move |evt| { let r = evt.element_coordinates(); mouse_coords.set(Some((r.x as i32, r.y as i32))); if let Some((_, _, sx, sy)) = device_info.read().as_ref() { let (cx, cy) = calculate_device_coords(r, *sx, *sy); device_coords.set(Some((cx, cy))); } },
                        onmouseleave: move |_| { mouse_coords.set(None); device_coords.set(None); is_swiping.set(false); swipe_start.set(None); swipe_end.set(None); },
                        onmousedown: move |evt| { if let Some((_, _, sx, sy)) = device_info.read().as_ref() { let r = evt.element_coordinates(); let (sx0, sy0) = calculate_device_coords(r, *sx, *sy); is_swiping.set(true); swipe_start.set(Some((sx0, sy0))); swipe_end.set(None); } },
                        onmouseup: move |evt| { if *is_swiping.read() { if let Some((name, _, sx, sy)) = device_info.read().as_ref() { let r = evt.element_coordinates(); let (ex, ey) = calculate_device_coords(r, *sx, *sy); if let Some((sx0, sy0)) = *swipe_start.read() { let dx = (ex as i32 - sx0 as i32).abs(); let dy = (ey as i32 - sy0 as i32).abs(); let distance = ((dx*dx + dy*dy) as f32).sqrt(); let name_clone = name.clone(); let auto = *auto_update_on_touch.read(); if auto { is_loading_screenshot.set(true); } if distance < 10.0 { spawn(async move { let result = async move { match Adb::new_with_device(&name_clone).await { Ok(adb) => match adb.tap(sx0, sy0).await { Ok(_) => { if auto { tokio::time::sleep(tokio::time::Duration::from_millis(500)).await; match adb.screen_capture_bytes().await { Ok(bytes) => Ok((true, bytes.to_vec())), Err(e) => Err(format!("Screenshot failed: {}", e)), } } else { Ok((false, Vec::new())) } }, Err(e) => Err(format!("Tap failed: {}", e)), }, Err(e) => Err(format!("ADB connection failed: {}", e)), } }.await; match result { Ok((updated, bytes)) => { if updated { let b64 = base64_encode(&bytes); screenshot_data.set(Some(b64)); screenshot_bytes.set(Some(bytes)); screenshot_status.set(format!("✅ Tapped at ({},{}) - Screenshot updated", sx0, sy0)); is_loading_screenshot.set(false); } else { screenshot_status.set(format!("✅ Tapped at ({},{})", sx0, sy0)); } } Err(e) => { screenshot_status.set(format!("❌ {}", e)); is_loading_screenshot.set(false); } } }); } else { spawn(async move { let result = async move { match Adb::new_with_device(&name_clone).await { Ok(adb) => match adb.swipe(sx0, sy0, ex, ey, Some(300)).await { Ok(_) => { if auto { tokio::time::sleep(tokio::time::Duration::from_millis(800)).await; match adb.screen_capture_bytes().await { Ok(bytes) => Ok((true, bytes.to_vec())), Err(e) => Err(format!("Screenshot failed: {}", e)), } } else { Ok((false, Vec::new())) } }, Err(e) => Err(format!("Swipe failed: {}", e)), }, Err(e) => Err(format!("ADB connection failed: {}", e)), } }.await; match result { Ok((updated, bytes)) => { if updated { let b64 = base64_encode(&bytes); screenshot_data.set(Some(b64)); screenshot_bytes.set(Some(bytes)); screenshot_status.set(format!("✅ Swiped ({},{}) -> ({},{}) - Screenshot updated", sx0, sy0, ex, ey)); is_loading_screenshot.set(false); } else { screenshot_status.set(format!("✅ Swiped ({},{}) -> ({},{})", sx0, sy0, ex, ey)); } } Err(e) => { screenshot_status.set(format!("❌ {}", e)); is_loading_screenshot.set(false); } } }); } } is_swiping.set(false); swipe_start.set(None); swipe_end.set(None); } } } },
                    // removed device coords & status display (moved to InteractionInfo panel)
                    if loading { div { style: "position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); background: rgba(255, 68, 68, 0.95); color: white; padding: 15px 25px; border-radius: 25px; font-size: 1.2em; font-weight: bold; border: 2px solid white; box-shadow: 0 4px 20px rgba(0,0,0,0.5); z-index: 20;", "📸 LOADING..." } }
                }
            } else {
                div { style: "text-align:center; opacity:0.6; font-size:0.8em;", "No screenshot yet." }
            }
        }
    }
}
