use dioxus::prelude::*;
use crate::adb::Adb;

pub fn run_gui() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut status = use_signal(|| "Initializing...".to_string());
    let mut device_info = use_signal(|| None::<(String, u32, u32, u32)>);
    
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
            
            // Device info section
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
                
                // Action buttons
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
                                std::thread::spawn(move || {
                                    if let Ok(adb) = Adb::new_with_device(&name_clone) {
                                        match adb.screen_capture("gui-screenshot.png") {
                                            Ok(_) => println!("✅ Screenshot saved to gui-screenshot.png"),
                                            Err(e) => println!("❌ Screenshot failed: {}", e),
                                        }
                                    }
                                });
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