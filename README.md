# rust-android-adb-automation

Rust library, CLI, and Dioxus desktop GUI for Android device automation via ADB. Provides two interchangeable backends:

- `rust` (default): Pure Rust using the `adb_client` crate (no external `adb` binary required).
- `shell`: Legacy subprocess backend invoking external `adb` (requires Platform Tools in PATH).

See detailed crate documentation in `android-adb-run/README.md`.

## Structure

```
./
  android-adb-run/        # main crate (library + CLI + GUI)
    src/adb.rs            # AdbClient trait + Device
    src/adb_rust.rs       # Pure Rust backend
    src/adb_shell.rs      # External adb backend
    src/adb_backend.rs    # Runtime enum dispatch
    src/gui/              # Dioxus components
```

## Features (Summary)

- Device listing & connection
- Screen size detection
- PNG screen capture (bytes)
- Tap & swipe input
- Transport ID (shell backend)
- Async (Tokio) for responsiveness
- GUI: live screenshot, tap markers (centered), selection rectangle, mutually exclusive modes (auto-refresh vs selection)
- Runtime backend selection via `--impl=<rust|shell>` or env `ADB_IMPL`

## Backend Selection

```bash
./android-adb-run --gui                # GUI (default rust backend)
./android-adb-run --screenshot         # Take screenshot (rust backend)
./android-adb-run --screenshot --impl=shell
ADB_IMPL=shell ./android-adb-run --gui # Force shell backend in GUI
```

## Conditional Prerequisites

External adb only needed for `--impl=shell`:

```bash
sudo apt install adb  # or add platform-tools to PATH
```

General:

1. Android device with USB debugging enabled
2. USB or TCP/IP connection (e.g. `adb tcpip 5555; adb connect <ip>:5555` for shell backend)

## Trait Overview

```rust
pub trait AdbClient: Send + Sync {
    async fn list_devices() -> Result<Vec<Device>, String> where Self: Sized;
    async fn new_with_device(device_name: &str) -> Result<Self, String> where Self: Sized;
    async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String>;
    async fn tap(&self, x: u32, y: u32) -> Result<(), String>;
    async fn swipe(&self, x1: u32, y1: u32, x2: u32, y2: u32, duration: Option<u32>) -> Result<(), String>;
    fn screen_dimensions(&self) -> (u32, u32);
    fn device_name(&self) -> &str;
    fn transport_id(&self) -> Option<u32>;
}
```

## Example (Pure Rust Backend)

```rust
use android_adb_run::adb_rust::RustAdb;

#[tokio::main]
async fn main() -> Result<(), String> {
    let devices = RustAdb::list_devices().await?;
    let first = devices.first().ok_or("No devices")?;
    let adb = RustAdb::new_with_device(&first.name).await?;
    let (w,h) = adb.screen_dimensions();
    println!("{} {}x{}", adb.device_name(), w, h);
    let bytes = adb.screen_capture_bytes().await?;
    std::fs::write("shot.png", &bytes).map_err(|e| e.to_string())?;
    adb.tap(w/2, h/2).await?;
    Ok(())
}
```

## Example (Shell Backend)

```rust
use android_adb_run::adb_shell::AdbShell;

#[tokio::main]
async fn main() -> Result<(), String> {
    let devices = AdbShell::list_devices().await?;
    let first = devices.first().ok_or("No devices")?;
    let adb = AdbShell::new_with_device(&first.name).await?;
    let (w,h) = adb.screen_dimensions();
    adb.swipe(10,10,200,200, Some(300)).await?;
    Ok(())
}
```

## Development & AI Notes

- Edition 2024
- Async Tokio throughout
- Dioxus modular GUI components
- Planned: multi-device selection, backend instance caching, screenshot debounce

## License

MIT

