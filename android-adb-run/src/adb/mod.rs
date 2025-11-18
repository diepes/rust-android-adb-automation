// ADB module - Android Debug Bridge pure Rust implementation
// This module provides abstractions for communicating with Android devices
// using the pure Rust adb_client library.

pub mod backend;
pub mod rust_impl;
pub mod types;

// Re-export the main types and functions for easy access
pub use backend::AdbBackend;
pub use rust_impl::RustAdb;
pub use types::{AdbClient, Device, ImageCapture};
