# Debugging: Tap Timeout Not Registering After USB Disconnect

## Problem Report

The tap event `claim_1d_tap` after USB disconnect does not seem to register timeout or error.

## Expected Behavior

When USB is disconnected and a tap event (`claim_1d_tap`) fires:
1. Tap operation should timeout after 5 seconds
2. Error message should contain "timed out" or "timeout"
3. `is_disconnect_error()` should detect it
4. Automation should pause
5. GUI should show disconnect message

## Actual Behavior (Reported)

Tap event doesn't seem to register timeout or error.

## Verification Steps

### Step 1: Enable Debug Mode

Run with debug mode to see all internal messages:

```bash
cargo run -- --debug
```

Look for these messages in console:
- `⚡ Executing timed event 'claim_1d_tap': Tap { x: 350, y: 628 }`
- `❌ Timed event 'claim_1d_tap' failed: ...`
- `🔌 Device disconnect detected during timed event: ...`

### Step 2: Check Console Output

When you unplug USB, you should see within 5-15 seconds:

```
⚡ Executing timed event 'claim_1d_tap': Tap { x: 350, y: 628 }
❌ Timed event 'claim_1d_tap' failed: ADB tap failed: RustAdb: tap timed out after 5 seconds (device may be disconnected)
🔌 Device disconnect detected during timed event: ADB tap failed: ...
```

### Step 3: Verify Timeout Implementation

Check that `src/adb/rust_impl.rs` has the timeout:

```rust
// Around line 818-843
async fn tap(&self, x: u32, y: u32) -> Result<(), String> {
    let tap_future = tokio::task::spawn_blocking(move || { ... });
    
    match tokio::time::timeout(Duration::from_secs(5), tap_future).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => Err(format!("RustAdb: tap task failed: {e}")),
        Err(_) => Err("RustAdb: tap timed out after 5 seconds (device may be disconnected)".to_string()),
    }
}
```

✅ **CONFIRMED**: Timeout is in place (lines 838-843)

### Step 4: Verify Error Detection

Check that `is_disconnect_error()` detects "timeout":

```rust
// src/game_automation/fsm.rs, line 20
pub fn is_disconnect_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();
    error_lower.contains("device offline")
        || error_lower.contains("device not found")
        // ... other patterns ...
        || error_lower.contains("timed out")  // ← This catches our timeout
        || error_lower.contains("timeout")
}
```

✅ **CONFIRMED**: "timeout" and "timed out" patterns are checked

### Step 5: Verify FSM Error Handling

Check that errors from `execute_timed_event()` are caught:

```rust
// src/game_automation/fsm.rs, lines 679-716
for (event_id, event_type) in events_to_execute {
    if let Err(e) = self.execute_timed_event(&event_id, &event_type).await {
        debug_print!("❌ Timed event '{}' failed: {}", event_id, e);
        
        if is_disconnect_error(&e) {
            debug_print!("🔌 Device disconnect detected...");
            self.change_state(GameState::Paused).await;
            // ... send disconnect event ...
            return;
        }
    }
}
```

✅ **CONFIRMED**: Error handling is in place

## Possible Issues

### Issue 1: Debug Mode Not Enabled

**Symptom:** No console output showing tap execution or errors

**Solution:**
```bash
# Make sure you run with --debug flag
cargo run -- --debug
```

### Issue 2: Timing Window Too Small

**Symptom:** You unplug USB between tap events, not during

**Details:**
- `claim_1d_tap` fires every 10 seconds
- If you unplug between events, next event will timeout
- But you need to wait 5 seconds for timeout + up to 10 seconds for next event
- Total wait: up to 15 seconds

**Solution:** Be patient, wait up to 15 seconds after unplugging

### Issue 3: Device Already Disconnected Before Tap

**Symptom:** Quick failure instead of 5-second timeout

**What happens:**
```
USB unplugged → ADB server detects immediately → 
Next tap fails instantly with "device offline"
```

This is actually **correct behavior** - faster failure is better!

**Expected console output:**
```
❌ Timed event 'claim_1d_tap' failed: ADB tap failed: RustAdb: tap failed: device offline
🔌 Device disconnect detected during timed event: ...
```

### Issue 4: GUI Not Showing Error

**Symptom:** Error occurs but GUI doesn't update

**Check:**
1. Look at console output - is error being logged?
2. Check GUI status panel - should show "🔌 USB DISCONNECTED: ..."
3. Check action button - should say "Resume" not "Pause"

### Issue 5: Automation Not Paused

**Symptom:** Automation continues trying to tap after disconnect

**Check:**
```rust
// FSM should call this when disconnect detected:
self.change_state(GameState::Paused).await;
```

**Verification:** GUI button should change from "Pause" to "Resume"

## Diagnostic Test

Run this isolated test:

```bash
# Terminal 1: Build the test
cd android-adb-run
cargo build --example test_claim_tap_timeout

# Terminal 2: Run the test
cargo run --example test_claim_tap_timeout

# Follow on-screen instructions:
# 1. Test 1 will tap with device connected (should succeed)
# 2. Test 2 will prompt you to unplug USB
# 3. Unplug USB cable
# 4. Wait and observe timing
```

Expected output for Test 2:
```
🎯 Attempting tap at (350, 628) with USB unplugged...
   Expected: Timeout after 5 seconds
❌ Tap failed after 5.00s
   Error: RustAdb: tap timed out after 5 seconds (device may be disconnected)
   ✅ Timeout detected correctly!
   ✅ Timeout occurred at expected time (5+ seconds)
```

## Common Misunderstandings

### "Not seeing error" vs "Error not detected"

These are different:

1. **Not seeing error in GUI**
   - Console shows error but GUI doesn't update
   - Issue with event channel or GUI rendering
   
2. **Error not detected as disconnect**
   - Error occurs but `is_disconnect_error()` returns false
   - Wrong error pattern
   
3. **No error at all**
   - Tap hangs indefinitely
   - Timeout wrapper not working

### Fast vs Slow Failure

- **Fast (<1s):** ADB immediately knows device is offline
- **Slow (5s):** Timeout fires because command is blocking

Both are correct! Fast is actually better.

## Debug Checklist

- [ ] Run with `--debug` flag
- [ ] Check console for "⚡ Executing timed event 'claim_1d_tap'"
- [ ] Check console for "❌ Timed event 'claim_1d_tap' failed"
- [ ] Check console for "🔌 Device disconnect detected"
- [ ] Verify GUI shows "🔌 USB DISCONNECTED"
- [ ] Verify GUI button shows "Resume"
- [ ] Verify no more tap attempts after disconnect
- [ ] Check timing: disconnect detected within 5-15 seconds

## Still Not Working?

If after following all steps above, the timeout still doesn't work:

### Collect Debug Info

1. **Console Output**
   ```bash
   cargo run -- --debug 2>&1 | tee debug.log
   # Unplug USB when automation is running
   # Wait 20 seconds
   # Send the debug.log file
   ```

2. **Code Verification**
   ```bash
   # Check exact tap implementation
   grep -A 20 "async fn tap" src/adb/rust_impl.rs
   
   # Check error detection
   grep -A 15 "pub fn is_disconnect_error" src/game_automation/fsm.rs
   ```

3. **Version Check**
   ```bash
   git log --oneline -10
   git diff HEAD src/adb/rust_impl.rs
   git diff HEAD src/game_automation/fsm.rs
   ```

### Report Format

```
## Issue Report

**Build:** <git commit hash>
**OS:** <linux/mac/windows>
**Device:** <device model>

**Steps to reproduce:**
1. cargo run -- --debug
2. Wait for claim_1d_tap to fire (every 10 seconds)
3. Unplug USB between events
4. Wait 15 seconds

**Console output:**
<paste last 50 lines>

**Expected:**
Within 15 seconds: "🔌 Device disconnect detected"

**Actual:**
<what actually happened>
```

## Quick Fix Attempts

If debugging shows the timeout IS working but not detected:

### Fix 1: Add More Debug Output

```rust
// In src/game_automation/fsm.rs, line 679
if let Err(e) = self.execute_timed_event(&event_id, &event_type).await {
    println!("🔍 DEBUG: Error from {}: {}", event_id, e);  // Add this
    println!("🔍 DEBUG: is_disconnect_error = {}", is_disconnect_error(&e));  // Add this
    
    debug_print!(self.debug_enabled, "❌ Timed event '{}' failed: {}", event_id, e);
    // ... rest of code ...
}
```

### Fix 2: Force Pause on ANY Tap Error

Temporary workaround for testing:

```rust
// In src/game_automation/fsm.rs, around line 750
TimedEventType::Tap { x, y } => {
    if let Some(client) = &self.adb_client {
        let client_guard = client.lock().await;
        match client_guard.tap(*x, *y).await {
            Ok(()) => { /* success */ }
            Err(e) => {
                println!("🔍 DEBUG: Tap error: {}", e);  // Add this
                // Force disconnect detection for testing
                return Err(format!("ADB tap failed (forced disconnect): {}", e));
            }
        }
    }
}
```

## Summary

The code looks correct based on review:
✅ Timeout is implemented (5 seconds)
✅ Error detection includes "timeout" and "timed out"
✅ FSM pauses on disconnect
✅ GUI receives disconnect events

Most likely causes:
1. Debug mode not enabled (missing console output)
2. Timing window (need to wait up to 15 seconds)
3. Fast failure instead of timeout (which is actually good!)

Run with `--debug` and watch the console carefully!
