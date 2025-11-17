# ADB USB Disconnect Detection Tests

This directory contains test applications to verify that USB disconnect detection is working correctly in the ADB automation system.

## Test Applications

### 1. Basic Disconnect Test (`test_adb_disconnect.rs`)

A simple test that repeatedly takes screenshots and detects when the device is disconnected.

**Run:**
```bash
cargo run --example test_adb_disconnect
```

**What it does:**
- Connects to the ADB device
- Takes screenshots every 2 seconds
- Detects when the USB cable is unplugged
- Attempts to reconnect automatically

**Expected behavior:**
1. You'll see successful screenshot captures
2. When you unplug the USB cable, you'll see an error message
3. The test will identify it as a disconnect error
4. It will attempt to reconnect

### 2. Comprehensive Disconnect Test (`test_adb_disconnect_comprehensive.rs`)

A thorough test that checks multiple ADB operations to see which one detects the disconnect first.

**Run:**
```bash
cargo run --example test_adb_disconnect_comprehensive
```

**What it does:**
- Tests screenshots, taps, and shell commands
- Performs all three operations in each iteration
- Shows which operation detects the disconnect first
- Prompts for manual reconnection and verification

**Expected behavior:**
1. Each iteration tests three operations
2. When you unplug the USB, one of the operations will fail
3. The test identifies it as a disconnect
4. You can manually reconnect and verify the connection

## How to Use

### Prerequisites

1. **Connect an Android device via USB**
   ```bash
   adb devices
   ```
   Should show your device listed.

2. **Enable USB debugging** on the device
   - Settings → Developer Options → USB Debugging

3. **Authorize the computer** when prompted on the device

### Running the Tests

1. **Start with the basic test:**
   ```bash
   cd android-adb-run
   cargo run --example test_adb_disconnect
   ```

2. **Watch the output** - you should see successful operations

3. **Unplug the USB cable** while the test is running

4. **Observe the disconnect detection:**
   - You should see an error message
   - The message should be identified as a disconnect error
   - The test should attempt to reconnect

5. **For the comprehensive test:**
   ```bash
   cargo run --example test_adb_disconnect_comprehensive
   ```
   - Press Enter to start
   - Watch which operation detects the disconnect first
   - Follow the reconnection prompts

## What's Being Tested

These tests verify that the `is_disconnect_error()` function correctly identifies disconnect errors from various ADB operations:

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

## Integration with Main Automation

The disconnect detection tested here is integrated into the main automation system (`fsm.rs`):

1. **Screenshot operations** - Detects disconnects in `take_screenshot()`
2. **Tap operations** - Detects disconnects in `execute_timed_event()` for taps
3. **Event processing** - Pauses automation on disconnect in `process_timed_events()`
4. **GUI updates** - Shows disconnection state in the GUI

## Expected Results

✅ **PASS:** The test detects the USB disconnect and logs it clearly

❌ **FAIL:** The test doesn't detect the disconnect or crashes

## Troubleshooting

**"No devices found":**
- Check `adb devices` shows your device
- Make sure USB debugging is enabled
- Try unplugging and reconnecting the device

**"Permission denied":**
- Accept the authorization prompt on the device
- Try `adb kill-server && adb start-server`

**Test never detects disconnect:**
- Make sure you're using a USB cable (not wireless ADB)
- The device might have a long timeout
- Try unplugging while an operation is in progress

## Notes

- These tests use the `rust_adb` implementation (not the shell-based ADB)
- The tests are non-destructive - they only take screenshots and perform taps
- You can stop the tests anytime with Ctrl+C
