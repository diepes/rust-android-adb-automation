use dioxus::prelude::*;
use crate::adb::Adb;

// Simple base64 encoding function
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    
    for chunk in data.chunks(3) {
        let mut buf = [0u8; 3];
        for (i, &byte) in chunk.iter().enumerate() {
            buf[i] = byte;
        }
        
        let b = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32);
        
        result.push(CHARS[((b >> 18) & 63) as usize] as char);
        result.push(CHARS[((b >> 12) & 63) as usize] as char);
        result.push(if chunk.len() > 1 { CHARS[((b >> 6) & 63) as usize] as char } else { '=' });
        result.push(if chunk.len() > 2 { CHARS[(b & 63) as usize] as char } else { '=' });
    }
    
    result
}

pub fn run_gui() {
    use dioxus::desktop::{Config, WindowBuilder};
    
    let config = Config::new()
        .with_window(
            WindowBuilder::new()
                .with_title("Android ADB Automation")
                .with_decorations(false)
                .with_resizable(true)
                .with_inner_size(dioxus::desktop::LogicalSize::new(1000, 700))
        );
    
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(App);
}

#[component]
fn App() -> Element {
    let mut status = use_signal(|| "Initializing...".to_string());
    let mut device_info = use_signal(|| None::<(String, u32, u32, u32)>);
    let mut screenshot_data = use_signal(|| None::<String>);
    let mut screenshot_bytes = use_signal(|| None::<Vec<u8>>);
    let mut screenshot_status = use_signal(|| "".to_string());
    let mut mouse_coords = use_signal(|| None::<(i32, i32)>);
    let mut device_coords = use_signal(|| None::<(u32, u32)>);
    
    // Initialize ADB connection on first render
    use_effect(move || {
        match Adb::new(None) {
            Ok(adb) => {
                device_info.set(Some((
                    adb.device.name.clone(),
                    adb.transport_id,
                    adb.screen_x,
                    adb.screen_y,
                )));
                status.set("Connected".to_string());
            }
            Err(e) => {
                status.set(format!("Error: {}", e));
            }
        }
    });

    rsx! {
        div {
            style: "padding: 15px; font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; color: white;",
            
            // Header
            div {
                style: "text-align: center; margin-bottom: 20px;",
                h1 { 
                    style: "font-size: 1.8em; margin: 0; text-shadow: 2px 2px 4px rgba(0,0,0,0.3);",
                    "🤖 Android ADB Automation" 
                }
            }
            
            // Main content area with sidebar layout
            div {
                style: "display: flex; gap: 20px; align-items: flex-start;",
                
                // Left side - main content
                div {
                    style: "flex: 1; min-width: 0;",
                    
                    // Status section
                    div {
                        style: "background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 20px; border-radius: 15px; margin-bottom: 20px; border: 1px solid rgba(255,255,255,0.2);",
                        h2 { 
                            style: "margin-top: 0; color: #ffd700;",
                            "📱 Connection Status" 
                        }
                        p { 
                            style: "font-size: 1.1em; margin: 10px 0;",
                            "Status: {status.read()}" 
                        }
                    }
            
                    // Device info and actions section
                    if let Some((name, transport_id, screen_x, screen_y)) = device_info.read().clone() {
                        div {
                            style: "background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 20px; border-radius: 15px; margin-bottom: 20px; border: 1px solid rgba(255,255,255,0.2);",
                            h2 { 
                                style: "margin-top: 0; color: #90ee90;",
                                "📋 Device Information" 
                            }
                            div {
                        style: "display: grid; grid-template-columns: 1fr 1fr; gap: 15px; margin-top: 15px;",
                        div {
                            p { 
                                style: "margin: 5px 0; font-size: 1.1em;",
                                strong { "Device Name: " }
                                span { style: "color: #ffd700;", "{name}" }
                            }
                            p { 
                                style: "margin: 5px 0; font-size: 1.1em;",
                                strong { "Transport ID: " }
                                span { style: "color: #ffd700;", "{transport_id}" }
                            }
                        }
                        div {
                            p { 
                                style: "margin: 5px 0; font-size: 1.1em;",
                                strong { "Screen Width: " }
                                span { style: "color: #ffd700;", "{screen_x}px" }
                            }
                            p { 
                                style: "margin: 5px 0; font-size: 1.1em;",
                                strong { "Screen Height: " }
                                span { style: "color: #ffd700;", "{screen_y}px" }
                            }
                        }
                    }
                            }
                        
                        // Action buttons for connected device
                        div {
                            style: "background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 20px; border-radius: 15px; margin-bottom: 20px; border: 1px solid rgba(255,255,255,0.2);",
                            h2 { 
                                style: "margin-top: 0; color: #87ceeb;",
                                "🎮 Actions" 
                            }
                            div {
                        style: "display: flex; gap: 15px; flex-wrap: wrap; justify-content: center;",
                        button {
                            style: "background: linear-gradient(45deg, #28a745, #20c997); color: white; padding: 15px 25px; border: none; border-radius: 10px; cursor: pointer; font-size: 1.1em; font-weight: bold; box-shadow: 0 4px 15px rgba(0,0,0,0.2); transition: all 0.3s ease; min-width: 150px;",
                            onclick: move |_| {
                                let name_clone = name.clone();
                                
                                screenshot_status.set("📸 Taking screenshot...".to_string());
                                
                                // Take screenshot in memory without saving to disk
                                match Adb::new_with_device(&name_clone) {
                                    Ok(adb) => {
                                        match adb.screen_capture_bytes() {
                                            Ok(image_bytes) => {
                                                let base64_string = base64_encode(&image_bytes);
                                                screenshot_data.set(Some(base64_string));
                                                screenshot_bytes.set(Some(image_bytes));
                                                screenshot_status.set("✅ Screenshot captured in memory!".to_string());
                                            }
                                            Err(e) => {
                                                screenshot_status.set(format!("❌ Screenshot failed: {}", e));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        screenshot_status.set(format!("❌ ADB connection failed: {}", e));
                                    }
                                }
                            },
                            "📸 Take Screenshot"
                        }
                        
                        // Save to Disk button - only show if we have screenshot data
                        if screenshot_bytes.read().is_some() {
                            button {
                                style: "background: linear-gradient(45deg, #6f42c1, #563d7c); color: white; padding: 15px 25px; border: none; border-radius: 10px; cursor: pointer; font-size: 1.1em; font-weight: bold; box-shadow: 0 4px 15px rgba(0,0,0,0.2); transition: all 0.3s ease; min-width: 150px;",
                                onclick: move |_| {
                                    if let Some(image_bytes) = screenshot_bytes.read().as_ref() {
                                        // Generate filename with simple timestamp
                                        let timestamp = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs();
                                        let filename = format!("screenshot_{}.png", timestamp);
                                        
                                        match std::fs::write(&filename, image_bytes) {
                                            Ok(_) => {
                                                screenshot_status.set(format!("✅ Screenshot saved to {}", filename));
                                            }
                                            Err(e) => {
                                                screenshot_status.set(format!("❌ Failed to save: {}", e));
                                            }
                                        }
                                    }
                                },
                                "💾 Save to Disk"
                            }
                        }
                        
                        button {
                            style: "background: linear-gradient(45deg, #dc3545, #e74c3c); color: white; padding: 15px 25px; border: none; border-radius: 10px; cursor: pointer; font-size: 1.1em; font-weight: bold; box-shadow: 0 4px 15px rgba(0,0,0,0.2); transition: all 0.3s ease; min-width: 150px;",
                            onclick: move |_| {
                                std::process::exit(0);
                            },
                            "🚪 Exit Application"
                        }
                            }
                        }
                    } else {
                        // No device connected - show exit button
                        div {
                            style: "background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 20px; border-radius: 15px; margin-bottom: 20px; border: 1px solid rgba(255,255,255,0.2);",
                            h2 { 
                                style: "margin-top: 0; color: #ffb347;",
                                "⚠️ No Device Connected" 
                            }
                            p {
                                style: "font-size: 1.1em; margin: 15px 0; text-align: center;",
                                "Please connect an Android device with ADB enabled, or use the CLI version with specific device commands."
                            }
                            div {
                                style: "display: flex; justify-content: center; margin-top: 20px;",
                                button {
                                    style: "background: linear-gradient(45deg, #dc3545, #e74c3c); color: white; padding: 15px 25px; border: none; border-radius: 10px; cursor: pointer; font-size: 1.1em; font-weight: bold; box-shadow: 0 4px 15px rgba(0,0,0,0.2); transition: all 0.3s ease; min-width: 150px;",
                                    onclick: move |_| {
                                        std::process::exit(0);
                                    },
                                    "🚪 Exit Application"
                                }
                            }
                        }
                    }
                }
                
                // Right side - screenshot area
                if !screenshot_status.read().is_empty() {
                    div {
                        style: "flex: 0 0 400px; background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 15px; border-radius: 15px; border: 1px solid rgba(255,255,255,0.2); height: fit-content;",
                        if let Some(image_data) = screenshot_data.read().as_ref() {
                            div {
                                style: "text-align: center; position: relative;",
                                img {
                                    src: "data:image/png;base64,{image_data}",
                                    style: "max-width: 100%; max-height: 600px; border-radius: 10px; box-shadow: 0 4px 15px rgba(0,0,0,0.3); cursor: crosshair;",
                                    onmousemove: move |evt| {
                                        // Get mouse position relative to the element
                                        let element_rect = evt.element_coordinates();
                                        mouse_coords.set(Some((element_rect.x as i32, element_rect.y as i32)));
                                        
                                        // Calculate device coordinates based on image scaling
                                        if let Some((_, _, screen_x, screen_y)) = device_info.read().as_ref() {
                                            // The image is displayed with max-width: 100% and max-height: 600px
                                            // We need to calculate the actual scaling factor
                                            let max_display_width = 400.0; // The container is 400px wide
                                            let max_display_height = 600.0;
                                            
                                            // Calculate the scale to fit the image within the container while maintaining aspect ratio
                                            let image_aspect = *screen_x as f32 / *screen_y as f32;
                                            let container_aspect = max_display_width / max_display_height;
                                            
                                            let (actual_width, actual_height) = if image_aspect > container_aspect {
                                                // Image is wider than container aspect ratio, so width is constrained
                                                (max_display_width, max_display_width / image_aspect)
                                            } else {
                                                // Image is taller than container aspect ratio, so height is constrained
                                                (max_display_height * image_aspect, max_display_height)
                                            };
                                            
                                            // Calculate scale factors
                                            let scale_x = *screen_x as f32 / actual_width;
                                            let scale_y = *screen_y as f32 / actual_height;
                                            
                                            // Convert mouse coordinates to device coordinates
                                            let device_x = (element_rect.x as f32 * scale_x) as u32;
                                            let device_y = (element_rect.y as f32 * scale_y) as u32;
                                            
                                            // Clamp to device bounds
                                            let clamped_x = device_x.min(*screen_x - 1);
                                            let clamped_y = device_y.min(*screen_y - 1);
                                            
                                            device_coords.set(Some((clamped_x, clamped_y)));
                                        }
                                    },
                                    onmouseleave: move |_| {
                                        mouse_coords.set(None);
                                        device_coords.set(None);
                                    },
                                    onclick: move |evt| {
                                        // Calculate click coordinates and send tap command
                                        if let Some((name, _, screen_x, screen_y)) = device_info.read().as_ref() {
                                            let element_rect = evt.element_coordinates();
                                            
                                            // Calculate device coordinates (same logic as in onmousemove)
                                            let max_display_width = 400.0;
                                            let max_display_height = 600.0;
                                            
                                            let image_aspect = *screen_x as f32 / *screen_y as f32;
                                            let container_aspect = max_display_width / max_display_height;
                                            
                                            let (actual_width, actual_height) = if image_aspect > container_aspect {
                                                (max_display_width, max_display_width / image_aspect)
                                            } else {
                                                (max_display_height * image_aspect, max_display_height)
                                            };
                                            
                                            let scale_x = *screen_x as f32 / actual_width;
                                            let scale_y = *screen_y as f32 / actual_height;
                                            
                                            let device_x = (element_rect.x as f32 * scale_x) as u32;
                                            let device_y = (element_rect.y as f32 * scale_y) as u32;
                                            
                                            let clamped_x = device_x.min(*screen_x - 1);
                                            let clamped_y = device_y.min(*screen_y - 1);
                                            
                                            // Send tap command to device
                                            let name_clone = name.clone();
                                            match Adb::new_with_device(&name_clone) {
                                                Ok(adb) => {
                                                    match adb.tap(clamped_x, clamped_y) {
                                                        Ok(_) => {
                                                            screenshot_status.set(format!("✅ Tapped at ({}, {})", clamped_x, clamped_y));
                                                        }
                                                        Err(e) => {
                                                            screenshot_status.set(format!("❌ Tap failed: {}", e));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    screenshot_status.set(format!("❌ ADB connection failed: {}", e));
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                // Coordinate display
                                if let Some((device_x, device_y)) = device_coords.read().as_ref() {
                                    div {
                                        style: "position: absolute; top: 10px; right: 10px; background: rgba(0,0,0,0.8); color: white; padding: 8px 12px; border-radius: 6px; font-size: 0.9em; font-family: monospace;",
                                        "Device: {device_x}, {device_y}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // Footer
            div {
                style: "text-align: center; margin-top: 30px; opacity: 0.7;",
                p { 
                    style: "font-size: 0.9em;",
                    "Built with Rust 🦀 and Dioxus ⚛️" 
                }
            }
        }
    }
}