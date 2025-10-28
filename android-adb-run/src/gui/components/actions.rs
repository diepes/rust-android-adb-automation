// gui/components/actions.rs
use crate::adb_backend::AdbBackend;
use crate::gui::util::base64_encode;
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct ActionsProps {
    pub name: String,
    pub is_loading: Signal<bool>,
    pub screenshot_status: Signal<String>,
    pub screenshot_data: Signal<Option<String>>,
    pub screenshot_bytes: Signal<Option<Vec<u8>>>,
    pub auto_update_on_touch: Signal<bool>,
    pub select_box: Signal<bool>, // new signal for select box
    pub use_rust_impl: bool,
    pub screenshot_counter: Signal<u64>, // GUI-level counter
}

#[component]
pub fn Actions(props: ActionsProps) -> Element {
    let mut is_loading = props.is_loading;
    let mut screenshot_status = props.screenshot_status;
    let mut screenshot_data = props.screenshot_data;
    let mut screenshot_bytes = props.screenshot_bytes;
    let mut auto_update_on_touch = props.auto_update_on_touch;
    let mut select_box = props.select_box;
    let name = props.name.clone();
    let use_rust_impl = props.use_rust_impl;
    let mut screenshot_counter = props.screenshot_counter;
    rsx! {
        div { style: "background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 20px; border-radius: 15px; margin-bottom: 20px; border: 1px solid rgba(255,255,255,0.2);",
            h2 { style: "margin-top:0; color:#87ceeb;", "🎮 Actions" }
            div { style: "display:flex; gap:15px; flex-wrap:wrap; justify-content:center;",
                button { style: if *is_loading.read() { "background:linear-gradient(45deg,#ff6b35,#f7931e); color:white; padding:15px 25px; border:none; border-radius:10px; cursor:wait; font-size:1.1em; font-weight:bold; min-width:150px; animation:pulse 1.5s infinite;" } else { "background:linear-gradient(45deg,#28a745,#20c997); color:white; padding:15px 25px; border:none; border-radius:10px; cursor:pointer; font-size:1.1em; font-weight:bold; min-width:150px;" },
                    onclick: move |_| {
                        if *is_loading.read() { return; }
                        let name_clone = name.clone();
                        is_loading.set(true);
                        screenshot_status.set("📸 Taking screenshot...".to_string());
                        spawn(async move {
                            let start = std::time::Instant::now();
                            let result = async move {
                                match AdbBackend::new_with_device(&name_clone, use_rust_impl).await {
                                    Ok(client) => match client.screen_capture_bytes().await {
                                        Ok(bytes) => Ok(bytes),
                                        Err(e) => Err(format!("Screenshot failed: {}", e)),
                                    },
                                    Err(e) => Err(format!("ADB connection failed: {}", e)),
                                }
                            }.await;
                            match result {
                                Ok(bytes) => {
                                    let duration_ms = start.elapsed().as_millis();
                                    let counter_val = screenshot_counter.with_mut(|c| { *c += 1; *c });
                                    let b64 = base64_encode(&bytes);
                                    screenshot_data.set(Some(b64));
                                    screenshot_bytes.set(Some(bytes));
                                    screenshot_status.set(format!("✅ Screenshot #{} captured in {}ms", counter_val, duration_ms));
                                }
                                Err(e) => screenshot_status.set(format!("❌ {}", e)),
                            }
                            is_loading.set(false);
                        });
                    },
                    if *is_loading.read() { "📸 Taking..." } else { "📸 Screenshot" }
                }
                div { style: "display:flex; flex-direction:column; align-items:center; justify-content:center; margin:10px 0; gap:8px;",
                    div { style: "display:flex; align-items:center; gap:8px;",
                        input {
                            r#type: "checkbox",
                            id: "auto-update-checkbox",
                            checked: *auto_update_on_touch.read(),
                            onchange: move |evt| {
                                let checked = evt.value().parse().unwrap_or(false);
                                auto_update_on_touch.set(checked);
                                if checked { select_box.set(false); }
                            },
                            style: "width:18px; height:18px; cursor:pointer;"
                        }
                        label { r#for: "auto-update-checkbox", style: "font-size:1em; cursor:pointer; user-select:none;", "📱 Update on tap/swipe" }
                    }
                    div { style: "display:flex; align-items:center; gap:8px;",
                        input {
                            r#type: "checkbox",
                            id: "select-box-checkbox",
                            checked: *select_box.read(),
                            onchange: move |evt| {
                                let checked = evt.value().parse().unwrap_or(false);
                                select_box.set(checked);
                                if checked { auto_update_on_touch.set(false); }
                            },
                            style: "width:18px; height:18px; cursor:pointer;"
                        }
                        label { r#for: "select-box-checkbox", style: "font-size:1em; cursor:pointer; user-select:none;", "🟦 Select box" }
                    }
                }
                if screenshot_bytes.read().is_some() { button { style: "background:linear-gradient(45deg,#6f42c1,#563d7c); color:white; padding:15px 25px; border:none; border-radius:10px; cursor:pointer; font-size:1.1em; font-weight:bold; min-width:150px;",
                    onclick: move |_| { if let Some(bytes) = screenshot_bytes.read().clone() { spawn(async move { let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(); let filename = format!("screenshot_{}.png", ts); match tokio::fs::write(&filename, &bytes).await { Ok(_) => screenshot_status.set(format!("✅ Screenshot saved to {}", filename)), Err(e) => screenshot_status.set(format!("❌ Failed to save: {}", e)), } }); } }, "💾 Save to Disk" } }
                button { style: "background:linear-gradient(45deg,#dc3545,#e74c3c); color:white; padding:15px 25px; border:none; border-radius:10px; cursor:pointer; font-size:1.1em; font-weight:bold; min-width:150px;", onclick: move |_| { std::thread::spawn(|| std::process::exit(0)); }, "🚪 Exit Application" }
            }
        }
    }
}
