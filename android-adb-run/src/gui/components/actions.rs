// gui/components/actions.rs
use dioxus::prelude::*;
use crate::adb::Adb;
use crate::gui::util::base64_encode;

#[derive(Props, PartialEq, Clone)]
pub struct ActionsProps {
    pub name: String,
    pub is_loading: Signal<bool>,
    pub screenshot_status: Signal<String>,
    pub screenshot_data: Signal<Option<String>>,
    pub screenshot_bytes: Signal<Option<Vec<u8>>>,
    pub auto_update_on_touch: Signal<bool>,
}

#[component]
pub fn Actions(props: ActionsProps) -> Element {
    let mut is_loading = props.is_loading;
    let mut screenshot_status = props.screenshot_status;
    let mut screenshot_data = props.screenshot_data;
    let mut screenshot_bytes = props.screenshot_bytes;
    let mut auto_update_on_touch = props.auto_update_on_touch;
    let name = props.name.clone();
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
                            let result = async move {
                                match Adb::new_with_device(&name_clone).await {
                                    Ok(adb) => match adb.screen_capture_bytes().await { Ok(bytes) => Ok(bytes.to_vec()), Err(e) => Err(format!("Screenshot failed: {}", e)) },
                                    Err(e) => Err(format!("ADB connection failed: {}", e)),
                                }
                            }.await;
                            match result {
                                Ok(bytes) => {
                                    let b64 = base64_encode(&bytes);
                                    screenshot_data.set(Some(b64));
                                    screenshot_bytes.set(Some(bytes));
                                    screenshot_status.set("✅ Screenshot captured in memory!".to_string());
                                }
                                Err(e) => screenshot_status.set(format!("❌ {}", e)),
                            }
                            is_loading.set(false);
                        });
                    },
                    if *is_loading.read() { "📸 Taking..." } else { "📸 Take Screenshot" }
                }
                div { style: "display:flex; align-items:center; justify-content:center; margin:10px 0; gap:8px;",
                    input { r#type: "checkbox", id: "auto-update-checkbox", checked: *auto_update_on_touch.read(), onchange: move |evt| { auto_update_on_touch.set(evt.value().parse().unwrap_or(false)); }, style: "width:18px; height:18px; cursor:pointer;" }
                    label { r#for: "auto-update-checkbox", style: "font-size:1em; cursor:pointer; user-select:none;", "📱 Update on tap/swipe" }
                }
                if screenshot_bytes.read().is_some() { button { style: "background:linear-gradient(45deg,#6f42c1,#563d7c); color:white; padding:15px 25px; border:none; border-radius:10px; cursor:pointer; font-size:1.1em; font-weight:bold; min-width:150px;",
                    onclick: move |_| { if let Some(bytes) = screenshot_bytes.read().clone() { spawn(async move { let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(); let filename = format!("screenshot_{}.png", ts); match tokio::fs::write(&filename, &bytes).await { Ok(_) => screenshot_status.set(format!("✅ Screenshot saved to {}", filename)), Err(e) => screenshot_status.set(format!("❌ Failed to save: {}", e)), } }); } }, "💾 Save to Disk" } }
                button { style: "background:linear-gradient(45deg,#dc3545,#e74c3c); color:white; padding:15px 25px; border:none; border-radius:10px; cursor:pointer; font-size:1.1em; font-weight:bold; min-width:150px;", onclick: move |_| { std::thread::spawn(|| std::process::exit(0)); }, "🚪 Exit Application" }
            }
        }
    }
}
