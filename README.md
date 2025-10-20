# rust-android-adb-automation

Rust library and CLI tool for automating Android device control through the ADB (Android Debug Bridge).

## Project Structure

- **Library**: Core ADB automation functionality in `src/adb.rs`
- **CLI Tool**: Command-line interface in `src/main.rs` 
- **GUI Tool**: Desktop GUI using Dioxus (optional, see `DIOXUS_SETUP.md`)

## Features

- Device discovery and connection
- Screen size detection
- Screen capture (PNG format)
- Touch input simulation (tap, swipe)
- **GUI with interactive touch**: Click to tap, drag to swipe on live screenshots
- Automatic device connection via network
- Transport ID-based device selection
- Async/non-blocking operations for responsive GUI

## Developement and using AI

- Try to keep the rust code modular
- Use async(tokio) to keep gui responsive
- Try to follow dioxus best practices
- Use dioxuis-cli for interactive gui developement and fast reload

      cargo install dioxus-cli


## Building and Running

- ```cargo install dioxus-cli```

### Command-Line Tool

```bash
cd android-adb-run
cargo build
./target/debug/android-adb-run
```

### GUI Tool (Optional)

```bash
cd android-adb-run
./target/debug/android-adb-run --gui
# or simply
./target/debug/android-adb-run
```

#### GUI Features:
- **Real-time screenshot display** with live coordinate tracking
- **Click-to-tap**: Click anywhere on the screenshot to tap that location on the device
- **Drag-to-swipe**: Click and drag to perform swipe gestures
- **Visual feedback**: Red border and loading indicators during operations
- **Auto-update**: Optional automatic screenshot after tap/swipe operations
- **Async operations**: Non-blocking UI with truly async ADB commands

See `DIOXUS_SETUP.md` for GUI setup instructions.

## Adb Automation Functions

- `screen_capture(output_path: &str)`: Capture the current screen and save as PNG.
- `tap(x: u32, y: u32)`: Tap at the given (x, y) coordinates on the device screen.
- `swipe(x1: u32, y1: u32, x2: u32, y2: u32, duration: Option<u32>)`: Swipe from (x1, y1) to (x2, y2) with optional duration.
- Device selection and connection via transport_id or device name.
- Screen size detection and bounds checking for input events.

## Prerequisites

1. **ADB installed and in PATH**:
   ```bash
   # Install ADB (Android SDK Platform Tools)
   # On Ubuntu/Debian:
   sudo apt install adb
   
   # Or download from: https://developer.android.com/studio/command-line/adb
   ```

2. **Android device with USB debugging enabled**
3. **Device connected via USB or network**

## Usage Examples

### Basic Connection
```rust
use android_adb_run::adb::Adb;

// Connect to first available device
let adb = Adb::new(None)?;

// Connect to specific transport ID
let adb = Adb::new(Some("2"))?;

// Connect to device by name (will attempt connection if not found)
let adb = Adb::new_with_device("oneplus6:5555")?;
```

### Screen Capture
```rust
adb.screen_capture("screenshot.png")?;
```

### Touch Input (CLI)
```rust
// Tap at coordinates (540, 1000)
adb.tap(540, 1000)?;

// Swipe from top to bottom
adb.swipe(540, 500, 540, 1500, Some(300))?;
```

### Touch Input (GUI)
- **Tap**: Click on the screenshot image to tap that location
- **Swipe**: Click and drag to create swipe gestures
- **Gesture Detection**: Short movements (< 10px) = tap, longer movements = swipe
- **Visual Feedback**: Orange indicator shows swipe start position during gesture
- **Auto-Screenshot**: Optional refresh after each gesture (checkbox control)

## adb notes

 * Start game TheTower

       adb -t 8 shell monkey -p com.TechTreeGames.TheTower 1


