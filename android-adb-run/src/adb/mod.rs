// ADB module - Android Debug Bridge pure Rust implementation
// This module provides abstractions for communicating with Android devices
// using direct USB connection (no daemon required).

pub mod backend;
pub mod error;
pub mod types;
pub mod usb_impl;

#[cfg(test)]
mod tests;

// Re-export the main types and functions for easy access
pub use backend::AdbBackend;
pub use error::{AdbError, AdbResult};
pub use types::{AdbClient, Device, ImageCapture};
pub use usb_impl::UsbAdb;
