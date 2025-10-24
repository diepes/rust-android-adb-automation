// gui/mod.rs
// GUI module root for android-adb-run

pub mod util;
// pub
mod components {
    pub mod header;
    pub mod device_info;
    pub mod actions;
    pub mod screenshot_panel;
    pub mod interaction_info; // new panel for interaction status & coords
}
pub mod dioxus; // main app

// Optionally, re-export common GUI types/functions
// pub use screenshot::*;
// pub use controls::*;
