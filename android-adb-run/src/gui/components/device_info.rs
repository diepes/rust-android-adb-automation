// gui/components/device_info.rs
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct DeviceInfoProps {
    pub name: String,
    pub transport_id: u32,
    pub screen_x: u32,
    pub screen_y: u32,
    pub status_style: String,
    pub status_label: String,
}

#[component]
pub fn DeviceInfo(props: DeviceInfoProps) -> Element {
    rsx! {
        div { style: "background: rgba(255,255,255,0.1); backdrop-filter: blur(10px); padding: 20px; border-radius: 15px; margin-bottom: 20px; border: 1px solid rgba(255,255,255,0.2);",
            div { style: "display: flex; align-items: center; gap: 10px; margin: 0 0 5px 0;",
                h2 { style: "margin: 0; color: #90ee90;", "📋 Device Information" }
                span { style: "{props.status_style}", "{props.status_label}" }
            }
            div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 15px; margin-top: 15px;",
                div { p { style: "margin:5px 0; font-size:1.1em;", strong { "Device Name: " } span { style: "color:#ffd700;", "{props.name}" } } p { style: "margin:5px 0; font-size:1.1em;", strong { "Transport ID: " } span { style: "color:#ffd700;", "{props.transport_id}" } } }
                div { p { style: "margin:5px 0; font-size:1.1em;", strong { "Screen Width: " } span { style: "color:#ffd700;", "{props.screen_x}px" } } p { style: "margin:5px 0; font-size:1.1em;", strong { "Screen Height: " } span { style: "color:#ffd700;", "{props.screen_y}px" } } }
            }
        }
    }
}
