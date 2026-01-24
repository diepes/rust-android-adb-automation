# TODO

## Completed ✅

### USB Disconnect Detectiona

- ✅ Screenshot timeout (10 seconds)
- ✅ Tap timeout (5 seconds) - Fixed with spawn_blocking
- ✅ Swipe timeout (5 seconds) - Fixed with spawn_blocking
- ✅ Error detection (13 patterns including "timeout")
- ✅ Automatic state pause on disconnect
- ✅ GUI shows disconnect message
- ✅ User can reconnect and resume

**See:** `TAP_ERROR_FLOW.md`, `TAP_FREEZE_FIX.md`, `DISCONNECT_DETECTION_COMPLETE.md`

### Code Simplificationa

- ✅ Removed shell ADB implementation
- ✅ Simplified to pure Rust implementation only
- ✅ Removed `--impl` flag
- ✅ Cleaned up 95% of backend.rs code
- ✅ Type alias: `AdbBackend = RustAdb`

**See:** `ADB_SIMPLIFICATION.md`

### Device Reconnection

- ✅ Countdown indicator on "No Devices Connected" screen (5s countdown)
- ✅ Automatic device reconnection detection
- ✅ Show retry countdown during device search
- ✅ Automatic reconnection when USB plugged back in
- ✅ Auto-resume automation after reconnection

**See:** `src/gui/hooks/device_loop.rs:24-48`, `src/game_automation/fsm.rs:1452-1473`

## In Progress 🚧

_No active tasks_

## Future Enhancements 💡

### Reconnection

- [ ] Connection quality monitoring

### Timeout's

- [ ] Make timeout values configurable
- [ ] Add timeout for other shell commands
- [ ] Progressive timeout increase on slow devices

### Error Handling

- [ ] Retry logic for transient errors
- [ ] Graceful degradation for slow operations
- [ ] Better error categorization
