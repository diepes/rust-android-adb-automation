// filepath: android-adb-run/src/adb_impl.rs
pub trait AdbImplementationSelector {
    fn is_rust(&self) -> bool;
    fn impl_str(&self) -> &'static str {
        if self.is_rust() { "rust" } else { "shell" }
    }
}

impl AdbImplementationSelector for bool {
    fn is_rust(&self) -> bool {
        *self
    }
}
