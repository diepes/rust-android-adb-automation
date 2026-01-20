// gui/components/screenshot_panel.rs
use crate::adb::{AdbClient, AdbResult};
use crate::gui::dioxus_app::AppContext;
use crate::gui::hooks::{device_loop::decode_screenshot_to_rgb, start_template_matching_phase};
use crate::gui::util::base64_encode;
use dioxus::html::geometry::ElementPoint;
use dioxus::prelude::*;
use std::time::Instant;

#[derive(Clone, PartialEq)]
pub struct TapMarker {
    pub point: ElementPoint,
    pub timestamp: Instant,
}

#[component]
pub fn screenshot_panel() -> Element {
    let ctx = use_context::<AppContext>();

    // Access grouped signals via the new structure
    let mut screenshot_status = ctx.screenshot.status;
    let mut screenshot_data = ctx.screenshot.data;
    let mut screenshot_bytes = ctx.screenshot.bytes;
    let mut screenshot_counter = ctx.screenshot.counter;
    let mut is_loading_screenshot = ctx.screenshot.is_loading;

    let device_info = ctx.device.info;
    let mut device_coords = ctx.device.coords;

    let mut mouse_coords = ctx.interaction.mouse_coords;
    let mut is_swiping = ctx.interaction.is_swiping;
    let mut swipe_start = ctx.interaction.swipe_start;
    let mut swipe_end = ctx.interaction.swipe_end;
    let auto_update_on_touch = ctx.interaction.auto_update_on_touch;
    let select_box = ctx.interaction.select_box;
    let mut selection_start = ctx.interaction.selection_start;
    let mut selection_end = ctx.interaction.selection_end;
    let hover_tap_preview = ctx.interaction.hover_tap_preview;

    let automation_command_tx = ctx.automation.command_tx;

    let calculate_device_coords = ctx.calculate_device_coords;
    let mut tap_markers = ctx.tap_markers;
    let shared_adb_client = ctx.shared_adb_client;
    let loading = *is_loading_screenshot.read();

    use_effect(move || {
        spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                tap_markers.with_mut(|markers| {
                    markers.retain(|m| m.timestamp.elapsed().as_secs() < 30);
                });
            }
        });
    });

    let adjust_overlay = |start: ElementPoint, end: ElementPoint| {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let width = dx.abs();
        let height = dy.abs();
        let left_raw = if dx >= 0.0 { start.x } else { start.x - width };
        let top_raw = if dy >= 0.0 { start.y } else { start.y - height };
        let border = 8.0;
        let left = left_raw + border;
        let top = top_raw + border;
        (left, top, width, height)
    };

    const CURSOR_OFFSET: f64 = 11.0;

    let overlay_rect: Option<(i32, i32, i32, i32)> = if *select_box.read() {
        if let (Some(start), Some(end)) = (*selection_start.read(), *selection_end.read()) {
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

    let device_to_display =
        |device_x: u32, device_y: u32, screen_x: u32, screen_y: u32| -> (f32, f32) {
            if screen_x == 0 || screen_y == 0 {
                return (0.0, 0.0);
            }
            let max_content_width = 400.0;
            let max_content_height = 600.0;
            let border_px = 8.0;
            let image_aspect = screen_x as f32 / screen_y as f32;
            let container_aspect = max_content_width / max_content_height;
            let (content_w, content_h) = if image_aspect > container_aspect {
                (max_content_width, max_content_width / image_aspect)
            } else {
                (max_content_height * image_aspect, max_content_height)
            };
            let scale_x = content_w.max(1.0) / screen_x as f32;
            let scale_y = content_h.max(1.0) / screen_y as f32;
            let px = device_x as f32 * scale_x + border_px;
            let py = device_y as f32 * scale_y + border_px;
            (px, py)
        };

    let hover_preview_point = {
        let preview_opt = *hover_tap_preview.read();
        let device_opt = device_info.read().clone();
        match (preview_opt, device_opt) {
            (Some((px, py)), Some((_, _, sx, sy))) if sx > 0 && sy > 0 => {
                let (disp_x, disp_y) = device_to_display(px, py, sx, sy);
                Some((disp_x, disp_y))
            }
            _ => None,
        }
    };

    let hover_css = r#"
        @keyframes hover-pulse-ring { 0% { transform: translate(-50%, -50%) scale(1.0); opacity: 0.8; } 50% { transform: translate(-50%, -50%) scale(1.35); opacity: 0.65; } 100% { transform: translate(-50%, -50%) scale(1.0); opacity: 0.8; } }
        @keyframes hover-pulse-core { 0% { transform: translate(-50%, -50%) scale(0.9); opacity: 0.95; } 50% { transform: translate(-50%, -50%) scale(1.3); opacity: 0.8; } 100% { transform: translate(-50%, -50%) scale(0.9); opacity: 0.95; } }
    "#;

    rsx! {
        style { dangerous_inner_html: "{hover_css}" }
        div { style: "flex:0 0 400px; background:rgba(255,255,255,0.1); backdrop-filter:blur(10px); padding:15px; border-radius:15px; border:1px solid rgba(255,255,255,0.2); height:fit-content;",
            if let Some(image_data) = screenshot_data.read().as_ref() {
                div { style: "display:flex; justify-content:center;",
                    div { style: "position:relative; width:fit-content;",
                        if let Some((device_x, device_y)) = *device_coords.read() {
                            div {
                                style: "position: absolute; top: 8px; left: 50%; transform: translateX(-50%); z-index: 10; background: rgba(0,0,0,0.8); color: white; padding: 4px 8px; border-radius: 6px; font-size: 0.8em; font-weight: bold; pointer-events: none; border: 1px solid rgba(255,255,255,0.3);",
                                "({device_x}, {device_y})"
                            }
                        }
                        img {
                            src: "data:image/png;base64,{image_data}",
                            style: if loading { "max-width:100%; max-height:600px; border-radius:10px; cursor:crosshair; border:8px solid #ff4444; box-shadow:0 0 40px rgba(255,68,68,0.8); user-select:none;" } else { "max-width:100%; max-height:600px; border-radius:10px; cursor:crosshair; border:8px solid rgba(255,255,255,0.2); box-shadow:0 4px 15px rgba(0,0,0,0.3); user-select:none;" },
                            onmousemove: move |evt| {
                                let r = evt.element_coordinates();
                                mouse_coords.set(Some((r.x as i32, r.y as i32)));
                                if let Some((_, _, sx, sy)) = device_info.read().as_ref() {
                                    let (cx, cy) = calculate_device_coords(r, *sx, *sy);
                                    device_coords.set(Some((cx, cy)));
                                }
                                if *select_box.read() && selection_start.read().is_some() { let adj = ElementPoint { x: r.x - CURSOR_OFFSET, y: r.y - CURSOR_OFFSET, ..r }; selection_end.set(Some(adj)); }
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
                                    if let (Some(start), Some(end)) = (*selection_start.read(), *selection_end.read())
                                        && let Some((_, _, screen_x, screen_y)) = device_info.read().as_ref() {
                                        let (left, top, width, height) = adjust_overlay(start, end);
                                        let tl = ElementPoint { x: left, y: top, ..start };
                                        let br = ElementPoint { x: left + width, y: top + height, ..start };
                                        let (d_tl_x, d_tl_y) = calculate_device_coords(tl, *screen_x, *screen_y);
                                        let (d_br_x, d_br_y) = calculate_device_coords(br, *screen_x, *screen_y);
                                        screenshot_status.set(format!("🟦 Selected: ({},{}) to ({},{}) size {}x{}", d_tl_x, d_tl_y, d_br_x, d_br_y, (d_br_x - d_tl_x).max(1), (d_br_y - d_tl_y).max(1)));
                                    }
                                    return;
                                }

                                if *is_swiping.read() {
                                    if let Some((sx0, sy0)) = *swipe_start.read() {
                                        let r = evt.element_coordinates();
                                        if let Some((_, _, sx, sy)) = device_info.read().as_ref() {
                                            let (ex, ey) = calculate_device_coords(r, *sx, *sy);
                                            let distance = ((ex as i32 - sx0 as i32).pow(2) as f32 + (ey as i32 - sy0 as i32).pow(2) as f32).sqrt();
                                            let auto = *auto_update_on_touch.read();
                                            let already_loading = *is_loading_screenshot.read();
                                            let refresh_after = auto && !already_loading;
                                            if refresh_after { is_loading_screenshot.set(true); }

                                            if let Some(cmd_tx) = automation_command_tx.read().as_ref() {
                                                let _ = cmd_tx.try_send(crate::game_automation::AutomationCommand::RegisterTouchActivity);
                                            }

                                            let client_arc = match shared_adb_client.read().clone() {
                                                Some(client) => client,
                                                None => {
                                                    screenshot_status.set("❌ ADB client not connected".to_string());
                                                    if refresh_after { is_loading_screenshot.set(false); }
                                                    return;
                                                }
                                            };

                                            spawn(async move {
                                                let result: AdbResult<Option<(Vec<u8>, u128, u64)>> = async {
                                                    let client = client_arc.lock().await;
                                                    if distance < 10.0 {
                                                        client.tap(sx0, sy0).await?;

                                                        // Add marker at tap location
                                                        tap_markers.with_mut(|markers| {
                                                            markers.push(TapMarker {
                                                                point: r,
                                                                timestamp: Instant::now(),
                                                            });
                                                        });
                                                    } else {
                                                        client.swipe(sx0, sy0, ex, ey, Some(300)).await?;
                                                    }

                                                    if refresh_after {
                                                        tokio::time::sleep(tokio::time::Duration::from_millis(if distance < 10.0 { 500 } else { 800 })).await;
                                                        let start = std::time::Instant::now();
                                                        let bytes = client.screen_capture_bytes().await?;
                                                        let duration_ms = start.elapsed().as_millis();
                                                        let counter_val = screenshot_counter.with_mut(|c| { *c += 1; *c });
                                                        Ok(Some((bytes, duration_ms, counter_val)))
                                                    } else {
                                                        Ok(None)
                                                    }
                                                }.await;

                                                match result {
                                                    Ok(cap_opt) => {
                                                        if let Some((bytes, duration_ms, counter_val)) = cap_opt {
                                                            let bytes_clone = bytes.clone();
                                                            let b64 = tokio::task::spawn_blocking(move || base64_encode(&bytes_clone)).await.unwrap_or_default();
                                                            screenshot_data.set(Some(b64));
                                                            screenshot_bytes.set(Some(bytes.clone()));
                                                            screenshot_status.set(format!("✅ Action successful - Screenshot #{} ({}ms)", counter_val, duration_ms));
                                                            is_loading_screenshot.set(false);

                                                            // Phase 3: Start template matching for this screenshot
                                                            let bytes_for_matching = bytes.clone();
                                                            let rgb_decoded = tokio::task::spawn_blocking(move || {
                                                                decode_screenshot_to_rgb(&bytes_for_matching).ok()
                                                            }).await.ok().flatten();
                                                            start_template_matching_phase(bytes, rgb_decoded, screenshot_status, ctx.screenshot.status_history.clone());
                                                        } else {
                                                            screenshot_status.set("✅ Action successful".to_string());
                                                        }
                                                    }
                                                    Err(e) => {
                                                        screenshot_status.set(format!("❌ Action failed: {}", e));
                                                        if refresh_after { is_loading_screenshot.set(false); }
                                                    }
                                                }
                                            });
                                        }
                                    }
                                    is_swiping.set(false); swipe_start.set(None); swipe_end.set(None);
                                }
                            }
                        }
                        if let Some((ox, oy, ow, oh)) = overlay_rect {
                            div { style: format!("position:absolute; left:{ox}px; top:{oy}px; width:{ow}px; height:{oh}px; border:2px solid #4da3ff; background:rgba(77,163,255,0.12); box-shadow:0 0 10px rgba(77,163,255,0.5); pointer-events:none; z-index:10;"),
                                div { style: "position:absolute; right:0; bottom:0; background:rgba(0,0,0,0.55); color:#fff; font-size:10px; padding:2px 4px; border-top-left-radius:4px;", "{ow}x{oh}" }
                            }
                        }
                        if let Some((disp_x, disp_y)) = hover_preview_point {
                            div { style: format!("position:absolute; left:{disp_x}px; top:{disp_y}px; width:20px; height:20px; border:2px solid #ff2d2d; background:rgba(255,45,45,0.2); border-radius:50%; box-shadow:0 0 12px rgba(255,45,45,0.75); transform:translate(-50%, -50%); pointer-events:none; z-index:12; animation:hover-pulse-ring 1.6s ease-in-out infinite;"), }
                            div { style: format!("position:absolute; left:{disp_x}px; top:{disp_y}px; width:6px; height:6px; background:#ff4545; border-radius:50%; transform:translate(-50%, -50%); pointer-events:none; z-index:13; animation:hover-pulse-core 1.6s ease-in-out infinite;"), }
                        }
                        for marker in tap_markers.read().iter() {{
                            let marker_x = marker.point.x;
                            let marker_y = marker.point.y;
                            let age_secs = marker.timestamp.elapsed().as_secs_f32();
                            let opacity = (1.0f32 - (age_secs / 30.0f32)).clamp(0.0f32, 1.0f32);
                            rsx!{ div { style: format!("position:absolute; left:{marker_x}px; top:{marker_y}px; width:10px; height:10px; background:#ffffff; border:2px solid #ff4444; border-radius:50%; box-shadow:0 0 6px rgba(255,255,255,0.8); transform:translate(-50%, -50%); pointer-events:none; z-index:9; opacity:{opacity};"), } }
                        }}
                        if loading { div { style: "position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); background: rgba(255, 68, 68, 0.95); color: white; padding: 15px 25px; border-radius: 25px; font-size: 1.2em; font-weight: bold; border: 2px solid white; box-shadow: 0 4px 20px rgba(0,0,0,0.5); z-index: 20;", "📸 LOADING..." } }
                    }
                }
            } else {
                div { style: "display:flex; justify-content:center; align-items:center; min-height:300px;",
                    if loading {
                        div { style: "text-align:center; background: rgba(0, 123, 255, 0.95); color: white; padding: 25px 35px; border-radius: 20px; font-size: 1.1em; font-weight: bold; border: 3px solid white; box-shadow: 0 8px 25px rgba(0,0,0,0.4); transition: all 0.3s ease;", "📸 Loading initial screenshot..." }
                    } else {
                        div { style: "text-align:center; opacity:0.6; font-size:0.9em; color: #666;", "📱 No screenshot available yet" }
                    }
                }
            }
        }
    }
}
