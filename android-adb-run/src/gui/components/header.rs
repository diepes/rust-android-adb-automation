// gui/components/header.rs
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct HeaderProps {
    pub on_drag: EventHandler<MouseEvent>,
    pub on_close: EventHandler<MouseEvent>,
}

#[component]
pub fn Header(props: HeaderProps) -> Element {
    rsx! {
        div { style: "background: rgba(255,255,255,0.08); padding:6px 10px; border-radius:10px; display:flex; align-items:center; gap:8px; border:1px solid rgba(255,255,255,0.15);",
            h1 { style: "font-size:1.05em; margin:0; font-weight:600; text-shadow:1px 1px 2px rgba(0,0,0,0.35);", "🤖 Android ADB Automation" }
            button { style: "background: rgba(255,255,255,0.15); color:#fff; border:1px solid rgba(255,255,255,0.3); padding:3px 8px; border-radius:5px; font-size:0.65em; cursor:grab;",
                onmousedown: move |e| props.on_drag.call(e), "🖱️ Drag" }
            button { style: "background: linear-gradient(135deg,#ff4d4d,#d63333); color:#fff; border:1px solid rgba(255,255,255,0.35); padding:3px 8px; border-radius:5px; font-size:0.65em; cursor:pointer; font-weight:600;",
                onclick: move |_| { std::thread::spawn(|| std::process::exit(0)); }, "✖ Close" }
        }
    }
}
