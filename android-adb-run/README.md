# android-adb-run

Rust library, CLI, and Dioxus desktop GUI for automating Android devices. Supports a pure Rust ADB backend (no global `adb` binary needed) plus an optional legacy shell backend.

## Implementations

**USB Direct ADB Backend** (default): Pure Rust via `adb_client` crate with direct USB connection. No ADB daemon required.

Key features:
- Direct USB communication using `ADBUSBDevice`
- Persistent ADB key support (~/.android/adbkey)
- Automatic retry logic for connection handshake errors
- Authentication validation before proceeding
- Smart framebuffer handling:
  - Detects PNG/JPEG formats automatically
  - Supports RGB565, RGB, RGBA raw formats
  - Falls back to screencap when needed

Select backend via CLI flag or environment variable:
```bash
./android-adb-run --screenshot              # default USB direct
./android-adb-run --screenshot --impl=usb
ADB_IMPL=usb ./android-adb-run --gui        # explicit USB in GUI
```
If unset, GUI defaults to USB direct implementation.

## Features

- **Device Discovery**: Direct USB device detection with retry logic
- **Persistent Authentication**: Uses ~/.android/adbkey for seamless connections
- **Screen Capture**: Smart framebuffer handling
  - Auto-detects PNG/JPEG formats returned by device
  - Converts RGB565/RGB/RGBA raw formats to PNG
  - Falls back to screencap command when needed
- **Input Control**: Tap & swipe with precise coordinates
- **GUI Features**:
  - Live screenshot display
  - Tap markers (centered circles)
  - Selection rectangle for region capture
  - Swipe vs tap detection (>=10px threshold)
  - Auto-refresh toggle
- **Touch Monitoring**: Pauses automation on human interaction
- **Modular Architecture**: Dioxus components in `src/gui/components/`

## Project Layout

```
android-adb-run/
  src/
    adb/
      mod.rs          # Module exports and AdbClient trait
      types.rs        # Device, AdbClient trait, TouchActivityMonitor
      usb_impl.rs     # Direct USB implementation (ADBUSBDevice) - active backend
      backend.rs      # Type alias for UsbAdb
      selector.rs     # Backend selection logic
    gui/              # Dioxus GUI components
      dioxus_app.rs   # Main GUI application
      components/     # Modular UI components
    game_automation/  # FSM and image matching
    main.rs           # CLI entrypoint
  examples/
    test_adb_connection.rs      # Connection diagnostics
    test_adb_image_capture.rs   # Screenshot format testing
```

## Building

```bash
cargo build --release
```

Run GUI:
```bash
./target/release/android-adb-run --gui
# Or with debug output:
./target/release/android-adb-run --gui --debug
```

Take screenshot (saved as `cli-screenshot.png`):
```bash
./target/release/android-adb-run --screenshot
```

## Testing & Diagnostics

Two test examples are provided for troubleshooting:

### Test ADB Connection
```bash
cargo run --example test_adb_connection
```
Tests:
- ADB key existence
- USB device detection
- Connection with retry logic
- Authentication validation
- Screenshot capture with timeout

### Test Image Capture Methods
```bash
cargo run --example test_adb_image_capture
```
Tests:
- Framebuffer format detection (PNG/JPEG/RAW)
- Format conversion (RGB565/RGB/RGBA to PNG)
- Screencap PNG method
- Screencap JPEG method
- Saves test files for manual inspection

Output includes detailed diagnostics about bytes per pixel, format detection, and timing information.

## Prerequisites

### ADB Key Setup
The USB implementation requires a persistent ADB key for authentication:

```bash
# Generate ADB key (run once)
adb devices

# This creates ~/.android/adbkey and ~/.android/adbkey.pub
```

### Linux USB Permissions
Add udev rule for your device (example for OnePlus):

```bash
# Find your device's USB vendor ID
lsusb | grep -i android

# Create udev rule (replace 2a70 with your vendor ID)
echo 'SUBSYSTEM=="usb", ATTR{idVendor}=="2a70", MODE="0666", GROUP="plugdev"' | \
  sudo tee /etc/udev/rules.d/51-android.rules

# Reload udev rules
sudo udevadm control --reload-rules
sudo udevadm trigger

# Reconnect device
```

### First Connection
On first connection, you'll see "Allow USB debugging?" popup on device:
- Check "Always allow from this computer"
- Tap "Allow" within 10 seconds
- Connection will retry automatically if needed

## Screenshot Implementation Details

The framebuffer capture has been optimized to handle various device formats:

1. **Format Detection**: Checks for PNG/JPEG magic bytes first
2. **Raw Conversion**: Handles RGB565 (2 bpp), RGB (3 bpp), RGBA (4 bpp)
3. **Header Handling**: Tries different header sizes (0, 12, 16, 20, 24 bytes)
4. **Screencap Fallback**: Falls back to `screencap -p` if framebuffer fails

Debug output (with `--debug` flag) shows format analysis:
```
DEBUG: Framebuffer analysis:
  Screen dimensions: 1080x2280 = 2462400 pixels
  Data length: 9854054 bytes
  Ratio: 4.00 bytes per pixel
  Detected format: 4 bytes per pixel (RGBA)
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

- ✅ Pause automation on manual events e.g. `adb shell getevent -lt /dev/input/event2` (COMPLETED: Smart touch device detection with `getevent -p` parsing)
- Improve Rust backend device naming (currently mirrors identifier string).
- Multi-device selection UI (instead of picking first automatically).
- Cache single backend instance in GUI to reduce repeated connections.
- Debounce screenshot auto-refresh.
- Document env var `ADB_IMPL` in more detail (already set by CLI launcher).
- Security: replace unsafe `set_var` once stable edition APIs allow.

## Touch Activity Monitoring

The application now includes intelligent touch activity detection that automatically pauses automation when human interaction is detected:

### Smart Device Detection
Uses `adb shell getevent -p` to intelligently identify the correct touchscreen device:
- **Vendor Priority**: Synaptics (100), Atmel/Goodix/Focaltech (90), Cypress/Elan (80)
- **Generic Detection**: "touch" (50), "screen" (40), "panel" (30), "ts" (20)
- **Device Avoidance**: Excludes buttons, audio jacks, GPIO, and other non-touch devices

Example device selection:
```
🔍 Parsing getevent -p output for touch devices...
  📱 Found touch device: /dev/input/event2 (name: 'synaptics,s3320', score: 100)
✅ Selected touch device: /dev/input/event2 (score: 100)
```

### Touch Monitoring Features
- **Background Monitoring**: Continuous `getevent` monitoring for touch events
- **30-Second Timeout**: Resumes automation after 30 seconds of no touch activity
- **Visual Feedback**: GUI shows pause/resume state with prominent indicators
- **Debug Output**: Comprehensive logging when `--debug` flag is used
- **Cross-Platform**: Works with both Rust and Shell ADB backends

## Development Notes

- Edition 2024
- Tokio for async
- Dioxus for desktop GUI
- Selection rectangle & tap marker overlay adjustments (translate centering & hotspot offset)

## License

MIT
