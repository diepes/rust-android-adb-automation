// Simplified ADB backend - Pure Rust implementation only
// The shell implementation has been removed as the pure Rust implementation is now stable and working well.

use super::rust_impl::RustAdb;

/// AdbBackend is now just a type alias for RustAdb for backward compatibility
/// This allows existing code to continue using AdbBackend without changes
pub type AdbBackend = RustAdb;

// Re-export Backend alias for backward compatibility
pub use AdbBackend as Backend;
