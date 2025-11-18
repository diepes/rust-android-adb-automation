# TODO

## Completed ✅

### USB Disconnect Detection
- ✅ Screenshot timeout (10 seconds)
- ✅ Tap timeout (5 seconds) - Fixed with spawn_blocking
- ✅ Swipe timeout (5 seconds) - Fixed with spawn_blocking
- ✅ Error detection (13 patterns including "timeout")
- ✅ Automatic state pause on disconnect
- ✅ GUI shows disconnect message
- ✅ User can reconnect and resume

**See:** `TAP_ERROR_FLOW.md`, `TAP_FREEZE_FIX.md`, `DISCONNECT_DETECTION_COMPLETE.md`

## In Progress 🚧

### Device Reconnection
- ⏳ Add countdown indicator on "No Devices Connected" screen
- TODO: Automatic device reconnection detection
- TODO: Show retry countdown (currently retries every 3 seconds silently)

## Future Enhancements 💡

### Reconnection
- [ ] Automatic reconnection when USB plugged back in
- [ ] Auto-resume after reconnection (configurable)
- [ ] Connection quality monitoring

### Timeouts
- [ ] Make timeout values configurable
- [ ] Add timeout for other shell commands
- [ ] Progressive timeout increase on slow devices

### Error Handling
- [ ] Retry logic for transient errors
- [ ] Graceful degradation for slow operations
- [ ] Better error categorization
