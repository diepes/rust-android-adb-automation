// gui/components/screenshot_panel.rs
use crate::AdbBackend;
use crate::adb::Adb;
use crate::adb::AdbClient;
use crate::adb_rust::RustAdb;
use crate::adb_shell::AdbShell;
use crate::gui::util::base64_encode;
use dioxus::html::geometry::ElementPoint;
use dioxus::prelude::*;

#[allow(unpredictable_function_pointer_comparisons)]
#[derive(Props, PartialEq, Clone)]
pub struct ScreenshotPanelProps {
    pub screenshot_status: Signal<String>,
    pub screenshot_data: Signal<Option<String>>,
    pub screenshot_bytes: Signal<Option<Vec<u8>>>,
    pub device_info: Signal<Option<(String, Option<u32>, u32, u32)>>,
    pub device_coords: Signal<Option<(u32, u32)>>,
    pub mouse_coords: Signal<Option<(i32, i32)>>,
    pub is_loading_screenshot: Signal<bool>,
    pub auto_update_on_touch: Signal<bool>,
    pub is_swiping: Signal<bool>,
    pub swipe_start: Signal<Option<(u32, u32)>>,
    pub swipe_end: Signal<Option<(u32, u32)>>,
    pub calculate_device_coords: fn(dioxus::html::geometry::ElementPoint, u32, u32) -> (u32, u32),
    pub select_box: Signal<bool>,
    pub selection_start: Signal<Option<ElementPoint>>,
    pub selection_end: Signal<Option<ElementPoint>>,
    pub tap_markers: Signal<Vec<ElementPoint>>,
}

#[component]
pub fn screenshot_panel(props: ScreenshotPanelProps) -> Element {
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
    let select_box = props.select_box;
    let mut selection_start = props.selection_start;
    let mut selection_end = props.selection_end;
    let mut tap_markers = props.tap_markers;
    let _status_text = screenshot_status.read().clone();

    // Helper to compute rectangle overlay adjusted for image border
    let adjust_overlay = |start: ElementPoint, end: ElementPoint| {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let width = dx.abs();
        let height = dy.abs();
        let left_raw = if dx >= 0.0 { start.x } else { start.x - width };
        let top_raw = if dy >= 0.0 { start.y } else { start.y - height };
        // compensate for 8px image border
        let border = 8.0;
        let left = left_raw + border;
        let top = top_raw + border;
        (left, top, width, height)
    };

    // Cursor hotspot adjustment (use center instead of bottom-right)
    const CURSOR_OFFSET: f64 = 11.0; // adjust if cursor size changes

    // Rectangle (not square) overlay state derived from selection signals
    let overlay_rect: Option<(i32, i32, i32, i32)> = if *select_box.read() {
        if let (Some(start), Some(end)) =
            (selection_start.read().clone(), selection_end.read().clone())
        {
            let (left, top, w, h) = adjust_overlay(start, end);
            Some((
                left.round() as i32,
                top.round() as i32,
                w.round() as i32,
                h.round() as i32,
            ))
        } else {
            None
        }
    } else {
        None
    };

    rsx! {
        div { style: "flex:0 0 400px; background:rgba(255,255,255,0.1); backdrop-filter:blur(10px); padding:15px; border-radius:15px; border:1px solid rgba(255,255,255,0.2); height:fit-content;",
            if let Some(image_data) = screenshot_data.read().as_ref() {
                div { style: "display:flex; justify-content:center;",
                    div { style: "position:relative; width:fit-content;",
                        img { src: "data:image/png;base64,{image_data}",
                            style: if loading { "max-width:100%; max-height:600px; border-radius:10px; cursor:crosshair; border:8px solid #ff4444; box-shadow:0 0 40px rgba(255,68,68,0.8); user-select:none;" } else { "max-width:100%; max-height:600px; border-radius:10px; cursor:crosshair; border:8px solid rgba(255,255,255,0.2); box-shadow:0 4px 15px rgba(0,0,0,0.3); user-select:none;" },
                            onmousemove: move |evt| {
                                let r = evt.element_coordinates();
                                mouse_coords.set(Some((r.x as i32, r.y as i32)));
                                if let Some((_, _, sx, sy)) = device_info.read().as_ref() {
                                    let (cx, cy) = calculate_device_coords(r, *sx, *sy);
                                    device_coords.set(Some((cx, cy)));
                                }
                                if *select_box.read() { if selection_start.read().is_some() { let adj = ElementPoint { x: r.x - CURSOR_OFFSET, y: r.y - CURSOR_OFFSET, ..r }; selection_end.set(Some(adj)); } }
                            },
                            onmouseleave: move |_| {
                                mouse_coords.set(None); device_coords.set(None); is_swiping.set(false); swipe_start.set(None); swipe_end.set(None);
                                if *select_box.read() { selection_start.set(None); selection_end.set(None); }
                            },
                            onmousedown: move |evt| {
                                if *select_box.read() {
                                    let r = evt.element_coordinates(); let adj = ElementPoint { x: r.x - CURSOR_OFFSET, y: r.y - CURSOR_OFFSET, ..r }; selection_start.set(Some(adj)); selection_end.set(None);
                                } else if let Some((_, _, sx, sy)) = device_info.read().as_ref() {
                                    let r = evt.element_coordinates(); let (sx0, sy0) = calculate_device_coords(r, *sx, *sy);
                                    is_swiping.set(true); swipe_start.set(Some((sx0, sy0))); swipe_end.set(None);
                                }
                            },
                            onmouseup: move |evt| {
                                if *select_box.read() {
                                    if let (Some(start), Some(end)) = (selection_start.read().clone(), selection_end.read().clone()) {
                                        if let Some((_, _, screen_x, screen_y)) = device_info.read().as_ref() {
                                            let dx = end.x - start.x; let dy = end.y - start.y;
                                            let width = dx.abs(); let height = dy.abs();
                                            let left = if dx >= 0.0 { start.x } else { start.x - width };
                                            let top = if dy >= 0.0 { start.y } else { start.y - height };
                                            let right = left + width; let bottom = top + height;
                                            let tl = ElementPoint { x: left, y: top, ..start };
                                            let br = ElementPoint { x: right, y: bottom, ..start };
                                            let (d_tl_x, d_tl_y) = calculate_device_coords(tl, *screen_x, *screen_y);
                                            let (d_br_x, d_br_y) = calculate_device_coords(br, *screen_x, *screen_y);
                                            screenshot_status.set(format!("🟦 Selected rectangle: ({},{}) to ({},{}) size {}x{}", d_tl_x, d_tl_y, d_br_x, d_br_y, (d_br_x - d_tl_x).max(1), (d_br_y - d_tl_y).max(1)));
                                        }
                                    }
                                    return;
                                }
                                if *is_swiping.read() {
                                    let swipe_start_val = swipe_start.read().clone();
                                    if let Some((name, _, sx, sy)) = device_info.read().as_ref() {
                                        let r = evt.element_coordinates(); let (ex, ey) = calculate_device_coords(r, *sx, *sy);
                                        if let Some((sx0, sy0)) = swipe_start_val {
                                            let dx = (ex as i32 - sx0 as i32).abs(); let dy = (ey as i32 - sy0 as i32).abs(); let distance = ((dx*dx + dy*dy) as f32).sqrt();
                                            let name_clone = name.clone(); let auto = *auto_update_on_touch.read(); if auto { is_loading_screenshot.set(true); }
                                            if distance < 10.0 {
                                                let raw_point = evt.element_coordinates(); tap_markers.with_mut(|v| v.push(raw_point));
                                                spawn(async move {
                                                    let impl_choice = std::env::var("ADB_IMPL").unwrap_or_else(|_| "rust".to_string());
                                                    let open = AdbBackend::connect_first(&impl_choice).await;
                                                    let result = async move { match open { Ok(client) => match client.tap(sx0, sy0).await { Ok(_) => { if auto { tokio::time::sleep(tokio::time::Duration::from_millis(500)).await; match client.screen_capture_bytes().await { Ok(bytes) => Ok((true, bytes)), Err(e) => Err(format!("Screenshot failed: {}", e)), } } else { Ok((false, Vec::new())) } }, Err(e) => Err(format!("Tap failed: {}", e)), }, Err(e) => Err(format!("ADB connection failed: {}", e)), } }.await;
                                                    match result { Ok((updated, bytes)) => { if updated { let b64 = base64_encode(&bytes); screenshot_data.set(Some(b64)); screenshot_bytes.set(Some(bytes)); screenshot_status.set(format!("✅ Tapped at ({},{}) - Screenshot updated", sx0, sy0)); is_loading_screenshot.set(false); } else { screenshot_status.set(format!("✅ Tapped at ({},{})", sx0, sy0)); } }, Err(e) => { screenshot_status.set(format!("❌ {}", e)); is_loading_screenshot.set(false); } }
                                                });
                                            } else {
                                                spawn(async move {
                                                    let impl_choice = std::env::var("ADB_IMPL").unwrap_or_else(|_| "rust".to_string());
                                                    let open = AdbBackend::connect_first(&impl_choice).await;
                                                    let result = async move { match open { Ok(client) => match client.swipe(sx0, sy0, ex, ey, Some(300)).await { Ok(_) => { if auto { tokio::time::sleep(tokio::time::Duration::from_millis(800)).await; match client.screen_capture_bytes().await { Ok(bytes) => Ok((true, bytes)), Err(e) => Err(format!("Screenshot failed: {}", e)), } } else { Ok((false, Vec::new())) } }, Err(e) => Err(format!("Swipe failed: {}", e)), }, Err(e) => Err(format!("ADB connection failed: {}", e)), } }.await;
                                                    match result { Ok((updated, bytes)) => { if updated { let b64 = base64_encode(&bytes); screenshot_data.set(Some(b64)); screenshot_bytes.set(Some(bytes)); screenshot_status.set(format!("✅ Swiped ({},{}) -> ({},{}) - Screenshot updated", sx0, sy0, ex, ey)); is_loading_screenshot.set(false); } else { screenshot_status.set(format!("✅ Swiped ({},{}) -> ({},{})", sx0, sy0, ex, ey)); } }, Err(e) => { screenshot_status.set(format!("❌ {}", e)); is_loading_screenshot.set(false); } }
                                                });
                                            }
                                            is_swiping.set(false); swipe_start.set(None); swipe_end.set(None);
                                        }
                                    }
                                }
                            },
                        }
                        if let Some((ox, oy, ow, oh)) = overlay_rect {
                            div { style: format!("position:absolute; left:{ox}px; top:{oy}px; width:{ow}px; height:{oh}px; border:2px solid #4da3ff; background:rgba(77,163,255,0.12); box-shadow:0 0 10px rgba(77,163,255,0.5); pointer-events:none; z-index:10;"),
                                div { style: "position:absolute; right:0; bottom:0; background:rgba(0,0,0,0.55); color:#fff; font-size:10px; padding:2px 4px; border-top-left-radius:4px;", "{ow}x{oh}" }
                            }
                        }
                        for p in tap_markers.read().iter() { {(|| {
                            let marker_x = p.x + 0.0;
                            let marker_y = p.y + 0.0;
                            rsx!{ div { style: format!("position:absolute; left:{marker_x}px; top:{marker_y}px; width:10px; height:10px; background:#ffffff; border:2px solid #ff4444; border-radius:50%; box-shadow:0 0 6px rgba(255,255,255,0.8); transform:translate(-50%, -50%); pointer-events:none; z-index:9;"), } }
                        })()} }
                        if loading { div { style: "position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); background: rgba(255, 68, 68, 0.95); color: white; padding: 15px 25px; border-radius: 25px; font-size: 1.2em; font-weight: bold; border: 2px solid white; box-shadow: 0 4px 20px rgba(0,0,0,0.5); z-index: 20;", "📸 LOADING..." } }
                    }
                }
            } else { div { style: "text-align:center; opacity:0.6; font-size:0.8em;", "No screenshot yet." } }
        }
    }
}
