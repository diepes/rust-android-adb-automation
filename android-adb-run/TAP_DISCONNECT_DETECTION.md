# Disconnect Detection via ADB Operation Errors

## Summary
Added disconnect detection to all ADB operations (tap, screenshot) that are executed during automation. This ensures that device disconnections are detected quickly, even if the automation is only performing taps and not screenshots.

## Changes Made

### Modified File: `src/game_automation/fsm.rs`

#### 1. Timed Event Execution Error Handling (Line ~662)
**Location:** `process_timed_events()` method where timed events are executed

**Before:**
```rust
if let Err(e) = self.execute_timed_event(&event_id, &event_type).await {
    let _ = self.event_tx
        .send(AutomationEvent::Error(format!("Timed event '{}' failed: {}", event_id, e)))
        .await;
}
```

**After:**
```rust
if let Err(e) = self.execute_timed_event(&event_id, &event_type).await {
    // Check if this is a disconnect error
    if is_disconnect_error(&e) {
        debug_print!(
            self.debug_enabled,
            "🔌 Device disconnect detected during timed event: {}",
            e
        );
        let _ = self.event_tx
            .send(AutomationEvent::DeviceDisconnected(format!(
                "Timed event '{}' failed: {}",
                event_id, e
            )))
            .await;
        return; // Stop processing further events on disconnect
    } else {
        let _ = self.event_tx
            .send(AutomationEvent::Error(format!("Timed event '{}' failed: {}", event_id, e)))
            .await;
    }
}
```

**Impact:** Detects disconnections when timed taps/screenshots fail during normal automation

#### 2. Manual Tap Trigger Error Handling (Line ~472)
**Location:** `AutomationCommand::TriggerTimedEvent` handler

**Before:**
```rust
if let Err(e) = client.tap(x, y).await {
    debug_print!(self.debug_enabled, "⚠️ Failed to execute tap ({}, {}): {}", x, y, e);
}
```

**After:**
```rust
if let Err(e) = client.tap(x, y).await {
    debug_print!(self.debug_enabled, "⚠️ Failed to execute tap ({}, {}): {}", x, y, e);
    
    // Check if this is a disconnect error
    if is_disconnect_error(&e) {
        debug_print!(
            self.debug_enabled,
            "🔌 Device disconnect detected during manual tap trigger: {}",
            e
        );
        let _ = self.event_tx
            .send(AutomationEvent::DeviceDisconnected(format!("Tap trigger failed: {}", e)))
            .await;
    }
}
```

**Impact:** Detects disconnections when manually triggered taps fail (via GUI "🔫" button)

#### 3. Image Recognition Tap Error Handling (Line ~932)
**Location:** `analyze_and_act()` method where image recognition taps occur

**Before:**
```rust
Err(e) => {
    return Err(format!("Failed to tap at ({}, {}): {}", tap_x, tap_y, e));
}
```

**After:**
```rust
Err(e) => {
    let error_msg = format!("Failed to tap at ({}, {}): {}", tap_x, tap_y, e);
    
    // Check if this is a disconnect error
    if is_disconnect_error(&error_msg) {
        debug_print!(
            self.debug_enabled,
            "🔌 Device disconnect detected during image recognition tap: {}",
            error_msg
        );
        let _ = self.event_tx
            .send(AutomationEvent::DeviceDisconnected(error_msg.clone()))
            .await;
    }
    
    return Err(error_msg);
}
```

**Impact:** Detects disconnections when image recognition triggers taps

## How It Works

### Detection Flow:
1. **Automation executes ADB operation** (tap, screenshot, etc.)
2. **Operation fails** (USB unplugged, device offline, etc.)
3. **Error propagates up** as `Err(String)`
4. **Error is checked** against `is_disconnect_error()` patterns:
   - "device offline"
   - "device not found"
   - "no devices"
   - "connection refused"
   - "broken pipe"
   - "connection reset"
   - "transport"
   - "closed"
   - "not connected"
   - "io error"
5. **If disconnect detected:**
   - Send `AutomationEvent::DeviceDisconnected`
   - GUI clears device info
   - Shows "🔌 Device Disconnected - Reconnecting..."
   - App exits after 2 seconds (exit code 1)
   - Wrapper script restarts app
   - Startup retry loop attempts reconnection

### Why This Approach is Better:
- ✅ **Simple**: No extra health check code needed
- ✅ **Effective**: Catches errors from actual operations
- ✅ **Fast**: Detects on first failed operation (tap happens frequently)
- ✅ **Reliable**: Works for all ADB backends (rust & shell)
- ✅ **Minimal overhead**: No periodic polling required

## Testing Results

### Test Scenario 1: Timed Tap with Disconnect
```
1. Start automation with timed tap every 5 seconds
2. Wait for first tap to succeed
3. Unplug USB cable
4. Next tap attempt fails → Disconnect detected immediately
5. GUI shows "Device Disconnected - Reconnecting..."
6. App restarts and retries connection
```

### Test Scenario 2: Screenshot with Disconnect
```
1. Start automation with screenshots every 10 minutes
2. Unplug USB before screenshot
3. Screenshot fails → Disconnect detected
4. Auto-reconnect triggered
```

### Test Scenario 3: Manual Tap Trigger with Disconnect
```
1. Start automation
2. Unplug USB
3. Click "🔫" trigger button on any timed event
4. Tap fails → Disconnect detected immediately
5. Auto-reconnect triggered
```

## Error Messages User Will See

**Before disconnect:**
- "✅ Connected via rust"
- "Running" (automation status)
- Timed events executing normally

**At disconnect:**
- "🔌 Device Disconnected - Reconnecting..."
- Device info panel disappears
- (2 second delay)
- App exits

**With wrapper script:**
- "[timestamp] Device disconnected, restarting in 2 seconds..."
- "[timestamp] Starting application..."
- "🔍 Looking for devices..."
- "❌ No devices found - retrying in 3s..."
- (Repeats every 3 seconds)

**After reconnect:**
- "📱 Found device: [device name]"
- "🔌 Connecting to [device]..."
- "✅ Connected via rust"
- Automation resumes

## Additional Context

This complements the existing disconnect detection in `take_screenshot()` and ensures that disconnections are caught regardless of which operation fails first. Since taps happen more frequently than screenshots in most automation scenarios, this significantly improves the disconnect detection responsiveness.

## Files Modified
- `src/game_automation/fsm.rs` (3 error handling locations updated)

## No New Files
All changes were made to existing error handling code paths.
