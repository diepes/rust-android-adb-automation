# USB Disconnect Detection Test Guide

## Overview

Two standalone test applications have been created to verify that USB disconnect detection is working correctly in the ADB automation system.

## Test Applications

### 1. Basic Disconnect Test

**File:** `examples/test_adb_disconnect.rs`

**Run:**
```bash
cd android-adb-run
cargo run --example test_adb_disconnect
```

**Features:**
- Continuously takes screenshots every 2 seconds
- Automatically detects USB disconnects
- Attempts automatic reconnection
- Tests the core `is_disconnect_error()` function

### 2. Comprehensive Disconnect Test

**File:** `examples/test_adb_disconnect_comprehensive.rs`

**Run:**
```bash
cd android-adb-run
cargo run --example test_adb_disconnect_comprehensive
```

**Features:**
- Tests multiple ADB operations (screenshots, taps, swipes)
- Shows which operation detects the disconnect first
- Prompts for manual reconnection
- More thorough validation

## Quick Start

1. **Connect your Android device via USB**
2. **Run the basic test:**
   ```bash
   cargo run --example test_adb_disconnect
   ```
3. **Wait for a few successful iterations**
4. **Unplug the USB cable**
5. **Observe the disconnect detection**

## What to Expect

### Successful Output:
```
📱 Connecting to ADB device...
✅ Connected to ADB device
📐 Screen dimensions: 1080x2400

🧪 Starting disconnect detection test...
Iteration 1: ✅ Screenshot successful (245678 bytes)
Iteration 2: ✅ Screenshot successful (245689 bytes)
Iteration 3: ❌ Error: device offline

🔌 DISCONNECT DETECTED!
Error message: 'device offline'

✅ Disconnect detection is working correctly!
```

## Error Detection Patterns

The tests verify detection of these disconnect errors:
- `device offline`
- `device not found`
- `no devices/emulators found`
- `connection refused`
- `broken pipe`
- `connection reset`
- `transport error`
- `connection closed`
- `not connected`
- `io error`

## Troubleshooting

**"No devices found":**
- Run `adb devices` to verify device is connected
- Enable USB debugging on device
- Accept authorization prompt

**Permission errors:**
- Check USB debugging authorization
- Try `adb kill-server && adb start-server`

**Test doesn't detect disconnect:**
- Ensure using USB cable (not wireless)
- Unplug cable during an operation
- Device may have long timeout

## Integration

These tests verify the same disconnect detection logic used in the main automation system (`fsm.rs`):

1. **Screenshot operations** - `take_screenshot()`
2. **Tap operations** - `execute_timed_event()` for taps  
3. **Timed events** - `process_timed_events()`
4. **Image recognition taps** - `analyze_and_act()`

## CI/CD Integration

These tests can be integrated into CI/CD pipelines for automated testing with physical devices or emulators.

## See Also

- `examples/README.md` - Detailed documentation
- `TAP_DISCONNECT_DETECTION.md` - Implementation details
- `src/game_automation/fsm.rs` - Main automation logic
