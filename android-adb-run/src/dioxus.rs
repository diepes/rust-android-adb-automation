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
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut status = use_signal(|| "Initializing...".to_string());
    let mut device_info = use_signal(|| None::<(String, u32, u32, u32)>);
    let mut screenshot_data = use_signal(|| None::<String>);
    let mut screenshot_status = use_signal(|| "".to_string());
    
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
            style: "padding: 20px; font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; color: white;",
            
            // Header
            div {
                style: "text-align: center; margin-bottom: 30px;",
                h1 { 
                    style: "font-size: 2.5em; margin-bottom: 10px; text-shadow: 2px 2px 4px rgba(0,0,0,0.3);",
                    "🤖 Android ADB Automation" 
                }
                p {
                    style: "font-size: 1.2em; opacity: 0.9;",
                    "Remote control and automation for Android devices"
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
                                
                                // Use a blocking approach instead of spawning a thread
                                match Adb::new_with_device(&name_clone) {
                                    Ok(adb) => {
                                        match adb.screen_capture("gui-screenshot.png") {
                                            Ok(_) => {
                                                // Read the PNG file and convert to base64
                                                match std::fs::read("gui-screenshot.png") {
                                                    Ok(image_bytes) => {
                                                        let base64_string = base64_encode(&image_bytes);
                                                        screenshot_data.set(Some(base64_string));
                                                        screenshot_status.set("✅ Screenshot captured!".to_string());
                                                    }
                                                    Err(e) => {
                                                        screenshot_status.set(format!("❌ Failed to read image: {}", e));
                                                    }
                                                }
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
                        style: "flex: 0 0 400px; background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 20px; border-radius: 15px; border: 1px solid rgba(255,255,255,0.2); height: fit-content;",
                        h2 { 
                            style: "margin-top: 0; color: #87ceeb; text-align: center;",
                            "📱 Screenshot" 
                        }
                        p {
                            style: "font-size: 1.1em; margin: 10px 0; text-align: center;",
                            "{screenshot_status.read()}"
                        }
                        
                        if let Some(image_data) = screenshot_data.read().as_ref() {
                            div {
                                style: "text-align: center; margin-top: 15px;",
                                img {
                                    src: "data:image/png;base64,{image_data}",
                                    style: "max-width: 100%; max-height: 600px; border-radius: 10px; box-shadow: 0 4px 15px rgba(0,0,0,0.3);"
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