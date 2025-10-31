// ADB module - Android Debug Bridge implementations and utilities
// This module provides abstractions for communicating with Android devices
// via both shell commands and pure Rust implementations.

pub mod types;
pub mod backend;
pub mod shell;
pub mod rust_impl;
pub mod selector;

// Re-export the main types and functions for easy access
pub use types::{AdbClient, Device, ImageCapture};
pub use backend::AdbBackend;
pub use shell::AdbShell;
pub use rust_impl::RustAdb;
pub use selector::AdbImplementationSelector;

// For backward compatibility, re-export AdbShell as Adb
pub use shell::AdbShell as Adb;
