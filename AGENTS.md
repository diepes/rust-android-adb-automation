# Code instructions related to the git repo

## Goal

Android phone automation over USB ADB using a pure-Rust `adb_client` crate — no ADB daemon required.

## Code Style

- Rust edition 2024; keep modules focused and structs lean.
- All errors use the `thiserror`-derived `AdbError` enum ([android-adb-run/src/adb/error.rs](android-adb-run/src/adb/error.rs)); use `AdbResult<T>` as the return type throughout `src/adb/`.
- Use the `debug_print!` macro (defined in [android-adb-run/src/lib.rs](android-adb-run/src/lib.rs)) instead of `println!` for conditional debug output.
- CLI flags are parsed manually in [android-adb-run/src/args.rs](android-adb-run/src/args.rs) — no `clap`. Follow the same pattern for new flags.
- TDD: write the test first, then implement. Keep each change small and independently verifiable.

## Architecture

```
android-adb-run/src/
  adb/           USB ADB layer — AdbBackend (UsbAdb), AdbClient trait, UsbCommand queue
  game_automation/ FSM event loop — GameAutomation, TimedEvent scheduler, match_image/
  gui/           Dioxus desktop GUI — AppContext, Signal bundles, components/
  template_matching/ Low-level template matching via imageproc (normalized cross-correlation)
  args.rs        CLI arg parsing (Mode::Gui | Mode::Screenshot, --debug, --timeout=N)
```

**Key data-flow:**
- All USB operations are serialized through a single `mpsc::Sender<UsbCommand>` inside `UsbAdb` to prevent concurrent USB access. See [android-adb-run/src/adb/types.rs](android-adb-run/src/adb/types.rs) for the `UsbCommand` enum.
- GUI → FSM: `mpsc::Sender<AutomationCommand>` (see `AutomationCommand` in [android-adb-run/src/game_automation/types.rs](android-adb-run/src/game_automation/types.rs)).
- FSM → GUI: Dioxus `Signal<T>` values bundled in `AutomationSignals`; GUI reads via `AppContext` in [android-adb-run/src/gui/dioxus_app.rs](android-adb-run/src/gui/dioxus_app.rs).
- `TouchActivityMonitor = Arc<RwLock<TouchActivityState>>` — pauses automation while a human is touching the screen.
- ADB protocol desyncs (CLSE errors) are detected by `AdbError::is_protocol_desync()` and trigger reconnect in the FSM.

## Build and Test

All commands run from `android-adb-run/`:

```bash
cd android-adb-run

# Build
cargo build

# Unit tests (no device needed)
cargo test --lib

# Run with GUI (requires Android device over USB)
cargo run

# Run with auto-exit timeout (preferred over shell `timeout`)
cargo run -- --timeout=25

# Release integration test
cargo run --release -- --timeout=25 2>&1 | grep -E "claim_1d_tap|Loop alive"

# Screenshot mode
cargo run -- --screenshot
```

## Project Conventions

- `AdbBackend` is a type alias for `UsbAdb`. Do not add a second concrete implementation without also updating the `AdbClient` trait.
- `TimedEvent` is the scheduling unit for all timed actions. Use `new_tap_seconds()` / `new_tap_hours()` constructors; tap interval is clamped to `[MIN_TAP_INTERVAL_SECONDS, MAX_TAP_INTERVAL_SECONDS]` (5 s … 6 h).
- Template images live under `android-adb-run/assets/test_images/`. `TemplateManager` rescans on `RescanTemplates` command.
- Framebuffer capture uses `framebuffer_bytes()` with fallback to `screencap -p` shell command.

## Timeout Flag

For testing automation timing without manual intervention — do NOT use shell `timeout`:
```bash
cargo run -- --timeout=25
```

## Test Coverage

#### Hardware Access Layer (22 tests)

- **Touch Activity Monitoring** (7 tests)
  - `test_touch_state_initial` - Initial state verification
  - `test_touch_activity_detection` - Touch event detection
  - `test_touch_activity_clear` - Touch state clearing
  - `test_touch_timeout_expiry` - Timeout expiration logic
  - `test_touch_activity_refresh` - Touch activity refresh
  - `test_concurrent_touch_monitoring` - Concurrent read/write safety
  - `test_touch_blocks_tap_execution` - Integration with tap queue

- **Tap Queue Processing** (5 tests)
  - `test_tap_queue_basic` - Basic queue operations
  - `test_tap_queue_ordering` - FIFO ordering preservation
  - `test_tap_queue_backpressure` - Channel backpressure handling
  - `test_tap_and_swipe_mixed_queue` - Mixed command types
  - `test_tap_queue_processor_shutdown` - Clean shutdown

- **Bounds & Validation** (1 test)
  - `test_tap_bounds_validation` - Screen coordinate validation

- **Screen Size Parsing** (3 tests)
  - `test_parse_screen_size` - Standard format parsing
  - `test_parse_screen_size_with_noise` - Parsing with extra output
  - `test_parse_screen_size_invalid` - Invalid input handling

- **Touch Event Detection** (1 test)
  - `test_touch_event_detection` - Event line pattern matching

- **Connection Logic** (3 tests)
  - `test_connection_retry_success_first_attempt` - Immediate success
  - `test_connection_retry_success_after_failures` - Retry on failure
  - `test_connection_retry_max_attempts_exceeded` - Max retry limit

- **Concurrency** (1 test)
  - `test_tap_queue_concurrent_with_screenshot` - Deadlock prevention

- **Framebuffer** (1 test)
  - `test_detect_framebuffer_format` - Format detection (RGB/RGBA/RGB565)

#### FSM & Timing (3 tests)

- `test_timed_event_interval_tracking` - Verifies TimedEvent state transitions
- `test_multiple_timed_events` - Tests independent event tracking  
- `test_lock_scope_prevents_deadlock` - Validates async lock patterns

