// gui/mod.rs
// GUI module root for android-adb-run

pub mod util;
pub mod components {
    pub mod actions;
    pub mod device_info;
    pub mod header;
    pub mod interaction_info;
    pub mod screenshot_panel; // new panel for interaction status & coords
}
pub mod dioxus_app; // renamed from dioxus

// Optionally, re-export common GUI types/functions
// pub use screenshot::*;
// pub use controls::*;
