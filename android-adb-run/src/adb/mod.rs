// ADB module - Android Debug Bridge implementations and utilities
// This module provides abstractions for communicating with Android devices
// via both shell commands and pure Rust implementations.

pub mod backend;
pub mod rust_impl;
pub mod selector;
pub mod shell;
pub mod types;

// Re-export the main types and functions for easy access
pub use backend::AdbBackend;
pub use rust_impl::RustAdb;
pub use selector::AdbImplementationSelector;
pub use shell::AdbShell;
pub use types::{AdbClient, Device, ImageCapture};

// For backward compatibility, re-export AdbShell as Adb
pub use shell::AdbShell as Adb;
