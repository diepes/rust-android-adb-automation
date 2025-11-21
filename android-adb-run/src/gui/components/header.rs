// gui/components/header.rs
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct HeaderProps {
    pub on_drag: EventHandler<MouseEvent>,
    pub on_minimize: EventHandler<MouseEvent>,
    pub on_maximize: EventHandler<MouseEvent>,
    pub on_close: EventHandler<MouseEvent>,
    pub runtime_days: Signal<f64>,
}

#[component]
pub fn Header(props: HeaderProps) -> Element {
    let runtime = *props.runtime_days.read();
    
    rsx! {
        div { 
            style: "background: rgba(255,255,255,0.08); padding:6px 10px; border-radius:10px; display:flex; align-items:center; gap:8px; border:1px solid rgba(255,255,255,0.15); cursor:grab; user-select:none;",
            onmousedown: move |e| props.on_drag.call(e),
            
            h1 { style: "font-size:1.05em; margin:0; font-weight:600; text-shadow:1px 1px 2px rgba(0,0,0,0.35); pointer-events:none;", "🤖 Android ADB Automation" }
            
            // Runtime display
            div { style: "background: rgba(0,0,0,0.25); color:#4dff88; border:1px solid rgba(77,255,136,0.3); padding:3px 10px; border-radius:6px; font-size:0.7em; font-weight:600; letter-spacing:0.5px; pointer-events:none;",
                "⏱️ {runtime:.3} days"
            }
            
            // Spacer to push window controls to the right
            div { style: "flex:1; pointer-events:none;" }
            
            // Window control buttons (minimize, maximize, close)
            div { style: "display:flex; gap:6px; pointer-events:auto;",
                // Minimize button
                button { 
                    style: "background: rgba(255,255,255,0.12); color:#fff; border:1px solid rgba(255,255,255,0.25); padding:3px 10px; border-radius:5px; font-size:0.7em; cursor:pointer; font-weight:600;",
                    onclick: move |e| { 
                        e.stop_propagation();
                        props.on_minimize.call(e);
                    },
                    "─" 
                }
                
                // Maximize/Restore button
                button { 
                    style: "background: rgba(255,255,255,0.12); color:#fff; border:1px solid rgba(255,255,255,0.25); padding:3px 10px; border-radius:5px; font-size:0.7em; cursor:pointer; font-weight:600;",
                    onclick: move |e| { 
                        e.stop_propagation();
                        props.on_maximize.call(e);
                    },
                    "□" 
                }
                
                // Close button
                button { 
                    style: "background: linear-gradient(135deg,#ff4d4d,#d63333); color:#fff; border:1px solid rgba(255,255,255,0.35); padding:3px 10px; border-radius:5px; font-size:0.7em; cursor:pointer; font-weight:600;",
                    onclick: move |e| { 
                        e.stop_propagation();
                        props.on_close.call(e);
                    },
                    "✖" 
                }
            }
        }
    }
}
