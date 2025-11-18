# ADB Tap Event Error Flow - Complete Trace

## Overview

This document traces how errors from ADB tap operations bubble up through the system to the GUI, with specific focus on disconnect detection and timeout handling.

## Complete Error Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. USER ACTION / TIMED EVENT                                    │
│    - User clicks on screenshot                                  │
│    - OR timed tap event fires                                   │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  v
┌─────────────────────────────────────────────────────────────────┐
│ 2. FSM: process_timed_events()                                  │
│    File: src/game_automation/fsm.rs:617                         │
│    - Collects ready events                                      │
│    - Calls execute_timed_event() for each                       │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  v
┌─────────────────────────────────────────────────────────────────┐
│ 3. FSM: execute_timed_event()                                   │
│    File: src/game_automation/fsm.rs:720                         │
│    - Matches event type (Screenshot, Tap, CountdownUpdate)      │
│    - For Tap: calls client_guard.tap(x, y).await                │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  v
┌─────────────────────────────────────────────────────────────────┐
│ 4. ADB Backend: tap()                                           │
│    File: src/adb/backend.rs:72                                  │
│    - Delegates to selected backend (Shell or RustAdb)           │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  v
┌─────────────────────────────────────────────────────────────────┐
│ 5. RustAdb Implementation: tap()                                │
│    File: src/adb/rust_impl.rs:807                               │
│    ┌─────────────────────────────────────────────────────────┐  │
│    │ A. Bounds Check                                         │  │
│    │    if x > screen_x || y > screen_y                      │  │
│    │    return Err("tap out of bounds")                      │  │
│    └─────────────────────────────────────────────────────────┘  │
│    ┌─────────────────────────────────────────────────────────┐  │
│    │ B. spawn_blocking (NEW FIX!)                            │  │
│    │    - Runs on blocking thread pool                       │  │
│    │    - Calls server_device.blocking_lock()                │  │
│    │    - Calls dev.shell_command(["input", "tap", x, y])    │  │
│    │                                                         │  │
│    │    Possible errors:                                     │  │
│    │    • "RustAdb: tap failed: {e}" (shell_command error)   │  │
│    │    • Device offline                                     │  │
│    │    • Connection refused                                 │  │
│    │    • Broken pipe                                        │  │
│    └─────────────────────────────────────────────────────────┘  │
│    ┌─────────────────────────────────────────────────────────┐  │
│    │ C. Timeout Wrapper (5 seconds)                          │  │
│    │    match tokio::time::timeout(5s, tap_future).await     │  │
│    │                                                         │  │
│    │    Ok(Ok(result))  → Success                            │  │
│    │    Ok(Err(e))      → "tap task failed: {e}"             │  │
│    │    Err(_)          → "tap timed out after 5 seconds"    │  │
│    └─────────────────────────────────────────────────────────┘  │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  │ Returns Result<(), String>
                  v
┌─────────────────────────────────────────────────────────────────┐
│ 6. FSM: execute_timed_event() - Error Handling                  │
│    File: src/game_automation/fsm.rs:757                         │
│                                                                 │
│    match client_guard.tap(x, y).await {                         │
│        Ok(()) => {                                              │
│            // Success path                                      │
│            send(AutomationEvent::TimedTapExecuted(id, x, y))    │
│        }                                                        │
│        Err(e) => {                                              │
│            return Err(format!("ADB tap failed: {}", e))         │
│        }                                                        │
│    }                                                            │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  │ Returns Err("ADB tap failed: ...")
                  v
┌─────────────────────────────────────────────────────────────────┐
│ 7. FSM: process_timed_events() - Error Classification           │
│    File: src/game_automation/fsm.rs:679                         │
│                                                                 │
│    if let Err(e) = execute_timed_event(...).await {             │
│        debug_print!("❌ Timed event '{}' failed: {}", id, e)    │
│                                                                 │
│        ┌─────────────────────────────────────────────────────┐  │
│        │ A. Check if disconnect error                        │  │
│        │    if is_disconnect_error(&e) {                     │  │
│        │        • Checks for 13 patterns:                    │  │
│        │          - "device offline"                         │  │
│        │          - "device not found"                       │  │
│        │          - "timeout" / "timed out" ← Our timeout!   │  │
│        │          - "connection refused"                     │  │
│        │          - "broken pipe"                            │  │
│        │          - ... and 8 more                           │  │
│        └─────────────────────────────────────────────────────┘  │
│                                                                 │
│        ┌─────────────────────────────────────────────────────┐  │
│        │ B. If DISCONNECT detected:                          │  │
│        │    1. change_state(GameState::Paused)               │  │
│        │    2. send(AutomationEvent::DeviceDisconnected(...))│  │
│        │    3. return (stop processing events)               │  │
│        └─────────────────────────────────────────────────────┘  │
│                                                                 │
│        ┌─────────────────────────────────────────────────────┐  │
│        │ C. If regular error (not disconnect):               │  │
│        │    send(AutomationEvent::Error(...))                │  │
│        │    continue processing next events                  │  │
│        └─────────────────────────────────────────────────────┘  │
│    }                                                            │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  │ Sends event via event_tx channel
                  v
┌─────────────────────────────────────────────────────────────────┐
│ 8. GUI Event Loop                                               │
│    File: src/gui/dioxus_app.rs:157                              │
│                                                                 │
│    while let Some(event) = event_rx.recv().await {              │
│        match event {                                            │
│            ...                                                  │
│        }                                                        │
│    }                                                            │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  v
┌─────────────────────────────────────────────────────────────────┐
│ 9. GUI Event Handlers                                           │
│    File: src/gui/dioxus_app.rs:340-365                          │
│                                                                 │
│    ┌─────────────────────────────────────────────────────────┐  │
│    │ A. AutomationEvent::Error(error)                        │  │
│    │    - Print to console (if debug)                        │  │
│    │    - Update screenshot_status_clone:                    │  │
│    │      "🤖 Automation error: {error}"                     │  │
│    │    - Error remains visible in GUI                       │  │
│    │    - Automation continues (error is non-fatal)          │  │
│    └─────────────────────────────────────────────────────────┘  │
│                                                                 │
│    ┌─────────────────────────────────────────────────────────┐  │
│    │ B. AutomationEvent::DeviceDisconnected(error)           │  │
│    │    1. Clear device info: device_info_clone.set(None)    │  │
│    │    2. Clear screenshots                                 │  │
│    │    3. Update screenshot_status_clone:                   │  │
│    │       "🔌 USB DISCONNECTED: {error} - Please reconnect" │  │
│    │    4. Update status:                                    │  │
│    │       "🔌 Device Disconnected - Automation Paused"      │  │
│    │    5. FSM already paused (state changed before)         │  │
│    └─────────────────────────────────────────────────────────┘  │
│                                                                 │
│    ┌─────────────────────────────────────────────────────────┐  │
│    │ C. AutomationEvent::StateChanged(GameState::Paused)     │  │
│    │    - Updates automation_state_clone signal              │  │
│    │    - Triggers button label changes:                     │  │
│    │      "Pause" → "Resume"                                 │  │
│    │    - Sent BEFORE DeviceDisconnected event               │  │
│    └─────────────────────────────────────────────────────────┘  │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  v
┌─────────────────────────────────────────────────────────────────┐
│ 10. GUI Visual Update                                           │
│     - Status bar shows disconnect message                       │
│     - Screenshot area cleared                                   │
│     - Device info hidden                                        │
│     - Action button shows "Resume" (paused state)               │
│     - User can reconnect USB and click Resume                   │
└─────────────────────────────────────────────────────────────────┘
```

## Error Message Transformations

### Path 1: Bounds Check Error

```
Input:  x=2000, y=3000, screen_x=1080, screen_y=1920
  ↓
rust_impl.rs:
  "RustAdb: tap out of bounds x=2000 y=3000"
  ↓
execute_timed_event():
  "ADB tap failed: RustAdb: tap out of bounds x=2000 y=3000"
  ↓
process_timed_events():
  is_disconnect_error() → false (not a disconnect)
  send(AutomationEvent::Error("Timed event 'tap_id' failed: ADB tap failed: ..."))
  ↓
GUI:
  screenshot_status: "🤖 Automation error: Timed event 'tap_id' failed: ..."
```

### Path 2: Device Offline Error (Immediate)

```
USB unplugged, shell_command returns error immediately
  ↓
rust_impl.rs spawn_blocking:
  dev.shell_command(...) → Error("device offline")
  ↓
rust_impl.rs tap():
  Ok(Err(e)) → "RustAdb: tap task failed: RustAdb: tap failed: device offline"
  ↓
execute_timed_event():
  "ADB tap failed: RustAdb: tap task failed: RustAdb: tap failed: device offline"
  ↓
process_timed_events():
  is_disconnect_error("...device offline...") → TRUE ✅
  change_state(Paused)
  send(AutomationEvent::DeviceDisconnected("Timed event 'tap_id' failed: ..."))
  ↓
GUI:
  1. StateChanged(Paused) → Button: "Resume"
  2. DeviceDisconnected → Status: "🔌 USB DISCONNECTED: ..."
```

### Path 3: Timeout Error (5 seconds)

```
USB unplugged, shell_command blocks waiting for device
  ↓
rust_impl.rs spawn_blocking:
  dev.shell_command(...) → [BLOCKING - waiting for I/O]
  ↓ (5 seconds pass)
  ↓
rust_impl.rs tap() timeout:
  Err(_) → "RustAdb: tap timed out after 5 seconds (device may be disconnected)"
  ↓
execute_timed_event():
  "ADB tap failed: RustAdb: tap timed out after 5 seconds..."
  ↓
process_timed_events():
  is_disconnect_error("...timed out...") → TRUE ✅
  change_state(Paused)
  send(AutomationEvent::DeviceDisconnected("Timed event 'tap_id' failed: ..."))
  ↓
GUI:
  1. StateChanged(Paused) → Button: "Resume"
  2. DeviceDisconnected → Status: "🔌 USB DISCONNECTED: ..."
```

## Key Components

### 1. Disconnect Error Detection

**File:** `src/game_automation/fsm.rs:20`

```rust
pub fn is_disconnect_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();
    error_lower.contains("device offline")
        || error_lower.contains("device not found")
        || error_lower.contains("no devices")
        || error_lower.contains("emulators found")
        || error_lower.contains("connection refused")
        || error_lower.contains("broken pipe")
        || error_lower.contains("connection reset")
        || error_lower.contains("transport")
        || error_lower.contains("closed")
        || error_lower.contains("not connected")
        || error_lower.contains("io error")
        || error_lower.contains("timed out")  // ← Catches our timeout!
        || error_lower.contains("timeout")
}
```

### 2. Timeout Implementation

**File:** `src/adb/rust_impl.rs:807`

```rust
async fn tap(&self, x: u32, y: u32) -> Result<(), String> {
    // Bounds check
    if x > self.screen_x || y > self.screen_y {
        return Err(format!("RustAdb: tap out of bounds x={x} y={y}"));
    }

    let server_device = Arc::clone(&self.server_device);
    
    // Run blocking operation on dedicated thread
    let tap_future = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut out: Vec<u8> = Vec::new();
        let mut dev = server_device.blocking_lock();
        let xs = x.to_string();
        let ys = y.to_string();
        dev.shell_command(&["input", "tap", &xs, &ys], &mut out)
            .map_err(|e| format!("RustAdb: tap failed: {e}"))?;
        Ok(())
    });

    // Wrap with timeout - can abandon blocking task
    match tokio::time::timeout(Duration::from_secs(5), tap_future).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => Err(format!("RustAdb: tap task failed: {e}")),
        Err(_) => Err("RustAdb: tap timed out after 5 seconds (device may be disconnected)".to_string()),
    }
}
```

### 3. State Pause Logic

**File:** `src/game_automation/fsm.rs:679`

```rust
// Execute ready events
for (event_id, event_type) in events_to_execute {
    if let Err(e) = self.execute_timed_event(&event_id, &event_type).await {
        debug_print!("❌ Timed event '{}' failed: {}", event_id, e);
        
        // Check if this is a disconnect error
        if is_disconnect_error(&e) {
            debug_print!("🔌 Device disconnect detected: {}", e);
            
            // CRITICAL: Pause automation first
            self.change_state(GameState::Paused).await;
            
            // Then notify GUI
            let _ = self.event_tx
                .send(AutomationEvent::DeviceDisconnected(format!(
                    "Timed event '{}' failed: {}",
                    event_id, e
                )))
                .await;
            
            return; // Stop processing further events
        } else {
            // Regular error - continue processing
            let _ = self.event_tx
                .send(AutomationEvent::Error(format!(
                    "Timed event '{}' failed: {}",
                    event_id, e
                )))
                .await;
        }
    }
}
```

## Event Ordering Guarantee

When a disconnect is detected, the FSM sends TWO events **in order**:

1. **`StateChanged(GameState::Paused)`** - Sent by `change_state()`
2. **`DeviceDisconnected(error)`** - Sent explicitly

The GUI receives them in order and:
- First updates button state to show "Resume"
- Then displays disconnect message

## Testing the Flow

### 1. Test Timeout Path

```bash
cargo run
# Wait for automation to start
# Unplug USB during tap
# After 5 seconds:
#   Console: "🔌 Device disconnect detected: ...timed out..."
#   GUI: Shows disconnect message + Resume button
```

### 2. Test Immediate Error Path

```bash
cargo run
# Ensure device is already disconnected
# Start automation
# Should see immediate error when first tap attempts
```

### 3. Debug with Verbose Logging

Enable debug mode to see all error transformations:

```rust
// In fsm.rs, set debug_enabled = true
// Or run with environment variable
RUST_LOG=debug cargo run
```

## Common Error Messages

| Scenario | Error String | Detected as Disconnect? |
|----------|-------------|------------------------|
| Bounds violation | "RustAdb: tap out of bounds x=2000 y=3000" | ❌ No - Regular error |
| USB unplugged (fast) | "RustAdb: tap failed: device offline" | ✅ Yes - Contains "offline" |
| USB unplugged (timeout) | "RustAdb: tap timed out after 5 seconds" | ✅ Yes - Contains "timed out" |
| Connection lost | "RustAdb: tap failed: broken pipe" | ✅ Yes - Contains "broken pipe" |
| Device not found | "RustAdb: tap failed: device not found" | ✅ Yes - Contains "device not found" |
| ADB not available | "ADB client not available" | ❌ No - Regular error |

## Summary

The error flow ensures:

✅ **All errors bubble up** - No errors are silently swallowed
✅ **Proper classification** - Disconnects vs regular errors
✅ **Automatic pause** - State changes before GUI notification
✅ **Clear feedback** - GUI shows appropriate message
✅ **Clean recovery** - User can reconnect and resume
✅ **No hangs** - Timeout ensures operations don't freeze
✅ **Thread safety** - spawn_blocking prevents blocking async executor

The system gracefully handles all error scenarios with appropriate user feedback.
