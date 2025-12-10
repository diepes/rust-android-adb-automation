# Code instructions related to the git repo

## Goal

* Mobile phone automation library, that uses rust adb_client module to connect to Android phone that is in debug mode over usb

## Coding guidance

* Keep the code modular and clean using rust structs where required

* Prefer that will simplify changes

* Try to use TTD where we create a test before we implement the item.

* Keep changes to small managable tasks that requires limited effort to achieve, trying to avoid big changes.

## Testing

### Timeout Flag
For testing automation timing without manual intervention:
```bash
cargo run -- --timeout=25
```
The app will auto-exit after 25 seconds. Use this instead of shell `timeout` command.

### Running Tests
```bash
# Unit tests
cargo test --lib

# Integration test with timeout
cargo run --release -- --timeout=25 2>&1 | grep -E "claim_1d_tap|Loop alive"
```

### Test Coverage

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

