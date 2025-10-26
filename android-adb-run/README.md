# android-adb-run

Rust library, CLI, and Dioxus desktop GUI for automating Android devices. Now supports a pure Rust ADB interaction layer – no global `adb` binary required for basic operations.

## Implementations

Two ADB backends are available behind the `AdbClient` trait:

- `shell` (legacy): Uses the external `adb` binary (must be in PATH).
- `rust` (default for CLI): Uses internal Rust implementation (`RustAdb`) to call needed commands (still invokes `adb` currently, but designed to move toward pure Rust client crate).

Select backend via CLI flag:

```bash
./android-adb-run --screenshot --impl=rust   # default
./android-adb-run --screenshot --impl=shell
```

GUI currently uses the shell backend; CLI defaults to `rust` backend.

## Features

- Device discovery
- Screen size detection
- PNG screen capture
- Tap & swipe input
- GUI: live screenshot, tap markers, selection rectangle
- Auto-update screenshot after interactions
- Modular Dioxus components (`src/gui/components/`)
- Trait-based backend abstraction (`AdbClient`)

## Project Layout

```
android-adb-run/
  src/
    adb.rs          # AdbClient trait + Device definition, re-exports shell impl as Adb
    adb_shell.rs    # Shell (external binary) implementation
    adb_rust.rs   # RustAdb implementation (toward pure client)
    gui/            # Dioxus GUI components
    main.rs         # CLI / entrypoint
```

## Building

```bash
cargo build
```

Run GUI (borderless window):
```bash
./target/debug/android-adb-run --gui
```
Or just run without flags for GUI.

Take screenshot (saved as `cli-screenshot.png`):
```bash
./target/debug/android-adb-run --screenshot
```

Explicit legacy shell backend:
```bash
./target/debug/android-adb-run --screenshot --impl=shell
```

## GUI Highlights

- Tap: click screenshot
- Swipe: drag (distance >=10px)
- Selection mode: draw rectangle (mutually exclusive with auto-update touch)
- Tap markers persist as circles
- Status panel updates with actions & selection info

## Backend Abstraction

`AdbClient` trait:
```rust
pub trait AdbClient {
    async fn list_devices() -> Result<Vec<Device>, String> where Self: Sized;
    async fn new_with_device(device_name: &str) -> Result<Self, String> where Self: Sized;
    async fn screen_capture_bytes(&self) -> Result<Vec<u8>, String>;
    async fn tap(&self, x: u32, y: u32) -> Result<(), String>;
    async fn swipe(&self, x1: u32, y1: u32, x2: u32, y2: u32, duration: Option<u32>) -> Result<(), String>;
    fn screen_dimensions(&self) -> (u32, u32);
    fn device_name(&self) -> &str;
}
```

## Moving Toward Pure Rust

Future step: replace shell calls with the `adb_rust` crate for true binary independence. Current `RustAdb` mirrors shell functionality to ease transition.

## Development Notes

- Edition 2024
- Tokio async for responsiveness
- Modular GUI components
- Rectangle selection overlays & coordinate mapping
- Persistent tap markers with center alignment correction

## License

MIT
