use std::path::PathBuf;
use thiserror::Error;

/// A specialized `Result` type for ADB operations.
pub type AdbResult<T> = Result<T, AdbError>;

/// The error type for all ADB-related operations.
#[derive(Debug, Error)]
pub enum AdbError {
    #[error("Failed to enumerate USB devices: {source}")]
    DeviceEnumerationFailed {
        #[from]
        source: adb_client::RustADBError,
    },

    #[error("ADB key not found at {path:?}. Please run 'adb devices' once to generate it.")]
    KeyNotFound { path: PathBuf },

    #[error("Failed to determine home directory for ADB key")]
    HomeDirectoryNotFound,

    #[error("USB device connection timed out after {duration:?}. Make sure to authorize USB debugging on your phone.")]
    ConnectionTimeout { duration: std::time::Duration },

    #[error("Failed to connect to USB device: {source}")]
    ConnectionFailed {
        source: adb_client::RustADBError,
    },

    #[error("Connection validation failed. The device may not be properly authorized: {source}")]
    ConnectionValidationFailed {
        source: adb_client::RustADBError,
    },

    #[error("Shell command '{command}' failed: {source}")]
    ShellCommandFailed {
        command: String,
        source: adb_client::RustADBError,
    },
    
    #[error("Operation timed out after {duration:?}: {description}")]
    Timeout {
        duration: std::time::Duration,
        description: String,
    },

    #[error("Task failed to complete: {source}")]
    JoinError {
        #[from]
        source: tokio::task::JoinError,
    },

    #[error("Could not parse screen size from 'wm size' output.")]
    ScreenSizeParseFailed,

    #[error("Framebuffer capture failed: {source}")]
    FramebufferCaptureFailed {
        source: adb_client::RustADBError,
    },

    #[error("Failed to convert framebuffer to PNG: {description}")]
    FramebufferToPngFailed { description: String },

    #[error("Failed to convert JPEG to PNG: {description}")]
    JpegToPngFailed { description: String },

    #[error("Tap coordinates are out of bounds: x={x}, y={y}")]
    TapOutOfBounds { x: u32, y: u32 },

    #[error("No touch-capable input devices found on the device")]
    NoTouchDeviceFound,

    #[error("This operation is not supported for USB devices: {operation}")]
    UnsupportedUsbOperation { operation: String },
}
