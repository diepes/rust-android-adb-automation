// gui/components/interaction_info.rs
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct InteractionInfoProps {
    pub device_coords: Signal<Option<(u32,u32)>>,
    pub screenshot_status: Signal<String>,
}

#[component]
pub fn InteractionInfo(props: InteractionInfoProps) -> Element {
    let status = props.screenshot_status.read().clone();
    let coords = props.device_coords.read().clone();
    rsx! {
        div { style: "background: rgba(255,255,255,0.08); padding:12px 16px; border-radius:12px; border:1px solid rgba(255,255,255,0.15); display:flex; flex-direction:column; gap:6px;",
            h3 { style: "margin:0 0 4px 0; font-size:0.85em; letter-spacing:0.5px; opacity:0.85;", "🖐 Interaction" }
            if let Some((x,y)) = coords { div { style: "font-size:0.75em; opacity:0.8;", {format!("Device tap: {x},{y}")} } } else { div { style: "font-size:0.7em; opacity:0.4;", "Hover over screenshot" } }
            if !status.is_empty() { div { style: "font-size:0.7em; line-height:1.2; opacity:0.75;", {status} } }
        }
    }
}
