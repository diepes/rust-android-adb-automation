# android-adb-run

Rust library, CLI, and Dioxus desktop GUI for automating Android devices. Supports a pure Rust ADB backend (no global `adb` binary needed) plus an optional legacy shell backend.

## Implementations

Two ADB backends behind the `AdbClient` trait:

- `rust` (default): Pure Rust via `adb_client` crate. Does not invoke external `adb`.
- `shell`: Uses external `adb` binary (must be in PATH). Provides transport_id field.

Select backend via CLI flag or environment variable:
```bash
./android-adb-run --screenshot --impl=rust   # default
./android-adb-run --screenshot --impl=shell
ADB_IMPL=shell ./android-adb-run --gui       # force shell in GUI
```
If unset, GUI inherits value set by `main.rs` (defaults to `rust`).

## Features

- Device discovery
- Screen size detection
- PNG screen capture (in‑memory bytes)
- Tap & swipe input
- GUI: live screenshot, tap markers (centered), selection rectangle, swipe vs tap detection
- Auto-update screenshot after interactions (toggle)
- Modular Dioxus components (`src/gui/components/`)
- Enum based backend dispatch (`AdbBackend`)

## Project Layout

```
android-adb-run/
  src/
    adb.rs          # AdbClient trait + Device definition (re-export shell impl as Adb)
    adb_shell.rs    # Shell implementation (external adb)
    adb_rust.rs     # Pure Rust implementation using adb_client
    adb_backend.rs  # Enum AdbBackend for runtime selection
    gui/            # Dioxus GUI components
    main.rs         # CLI / entrypoint (parses --impl)
```

## Building

```bash
cargo build
```

Run GUI:
```bash
./target/debug/android-adb-run --gui
```
Or simply run without flags for GUI (default mode).

Take screenshot (saved as `cli-screenshot.png`):
```bash
./target/debug/android-adb-run --screenshot
```

Explicit shell backend:
```bash
./target/debug/android-adb-run --screenshot --impl=shell
```

## Backend Notes

- Rust backend obtains devices and executes shell commands internally (screencap, wm size, input) via `ADBServerDevice.shell_command`.
- Shell backend validates external `adb` availability (`ensure_adb_available`) and surfaces friendly errors.

## Conditional Prerequisite

Only for `--impl=shell`:
```bash
sudo apt install adb  # or add platform-tools to PATH
```

## GUI Highlights

- Tap: click screenshot
- Swipe: drag (>=10px)
- Selection mode: draw rectangle (mutually exclusive with auto-refresh mode)
- Tap markers: centered circles with cursor hotspot offset
- Status area: device info + action feedback

## Backend Abstraction

`AdbClient` trait:
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
Runtime enum `AdbBackend` dispatches each call to selected implementation.

## Moving Forward / TODO

- Improve Rust backend device naming (currently mirrors identifier string).
- Multi-device selection UI (instead of picking first automatically).
- Cache single backend instance in GUI to reduce repeated connections.
- Debounce screenshot auto-refresh.
- Document env var `ADB_IMPL` in more detail (already set by CLI launcher).
- Security: replace unsafe `set_var` once stable edition APIs allow.

## Development Notes

- Edition 2024
- Tokio for async
- Dioxus for desktop GUI
- Selection rectangle & tap marker overlay adjustments (translate centering & hotspot offset)

## License

MIT
