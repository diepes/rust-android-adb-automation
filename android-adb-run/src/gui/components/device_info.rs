// gui/components/device_info.rs
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct DeviceInfoProps {
    pub name: String,
    pub transport_id: Option<u32>,
    pub screen_x: u32,
    pub screen_y: u32,
    pub status_style: String,
    pub status_label: String,
    pub runtime_days: f64,
}

#[component]
pub fn DeviceInfo(props: DeviceInfoProps) -> Element {
    let transport_display = props
        .transport_id
        .map(|v| v.to_string())
        .unwrap_or_else(|| "-".to_string());
    let runtime_badge = format!("⏱️ {:.3} days", props.runtime_days);
    rsx! {
        div { style: "background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 12px; border-radius: 12px; margin-bottom: 15px; border: 1px solid rgba(255,255,255,0.2);",
            div { style: "display: flex; align-items: center; gap: 8px; margin: 0 0 8px 0;",
                h2 { style: "margin: 0; color: #90ee90; font-size: 0.95em;", "📋 Device Information" }
                span { style: "{props.status_style}", "{props.status_label}" }
                span { style: "background: rgba(0,0,0,0.25); color:#4dff88; border:1px solid rgba(77,255,136,0.3); padding:3px 8px; border-radius:6px; font-size:0.75em; font-weight:600; letter-spacing:0.4px;", "{runtime_badge}" }
            }
            div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 10px;",
                div { p { style: "margin:3px 0; font-size:0.8em;", strong { "Device Name: " } span { style: "color:#ffd700;", "{props.name}" } } p { style: "margin:3px 0; font-size:0.8em;", strong { "Transport ID: " } span { style: "color:#ffd700;", "{transport_display}" } } }
                div { p { style: "margin:3px 0; font-size:0.8em;", strong { "Screen Width: " } span { style: "color:#ffd700;", "{props.screen_x}px" } } p { style: "margin:3px 0; font-size:0.8em;", strong { "Screen Height: " } span { style: "color:#ffd700;", "{props.screen_y}px" } } }
            }
        }
    }
}
