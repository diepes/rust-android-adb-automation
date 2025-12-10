# Architecture Refactor: Hybrid Signal/Channel Design

## Goal
Simplify communication between GUI and game automation backend by using direct signal updates instead of event channels.

## Current Architecture Issues
- ❌ Double indirection: Backend → Event channel → Event receiver → Signal updates
- ❌ Unnecessary event receiver task spawning
- ❌ Signal cloning overhead (10+ signals cloned for event receiver)
- ❌ Using `tokio::spawn` instead of Dioxus `spawn` (line 471)

## New Architecture (Option 2 - Hybrid)
- ✅ GUI → Backend: Keep command channel (discrete actions like start/stop)
- ✅ Backend → GUI: Direct signal updates (no event channel)
- ✅ Use Dioxus `spawn()` everywhere (not `tokio::spawn`)

## Implementation Steps

### Phase 1: Preparation
- [x] Document current architecture
- [ ] Create backup branch
- [ ] List all AutomationEvent types that need conversion to signal updates

### Phase 2: Modify GameAutomation struct
- [x] Add Signal fields to GameAutomation struct
  - screenshot_data: Signal<Option<String>>
  - screenshot_bytes: Signal<Option<Vec<u8>>
  - screenshot_status: Signal<String>
  - automation_state: Signal<GameState>
  - is_paused_by_touch: Signal<bool>
  - touch_timeout_remaining: Signal<Option<u64>>
  - timed_tap_countdown: Signal<Option<(String, u64)>>
  - timed_events_list: Signal<Vec<TimedEvent>>
  - device_info: Signal<Option<(String, Option<u32>, u32, u32)>>
  - status: Signal<String>
  - screenshot_counter: Signal<u64>
- [x] Remove event_tx: mpsc::Sender<AutomationEvent> field
- [x] Update GameAutomation::new() signature to accept signals

### Phase 3: Update fsm.rs (BLOCKED - API issue)
**ISSUE**: Dioxus Signal.set() requires &mut self, but our methods use &self.
**OPTIONS**:
1. Change all methods to &mut self (breaking change, lots of edits)
2. Use Signal interior mutability differently (investigate write())
3. Wrap signals in Arc<Mutex<>> (defeats purpose of signals)

### Phase 3 Progress (26/26 event_tx.send replaced)
- [x] Replace TemplatesUpdated (removed - not needed)
- [x] Replace StateChanged with automation_state.set()
- [x] Replace Error in initialize_adb with screenshot_status.set()
- [ ] Replace ScreenshotTaken with screenshot_bytes + screenshot_data (needs base64)
- [ ] Replace DeviceDisconnected with device_info + status signals
- [ ] Replace all remaining Error events
- [ ] Replace ManualActivityDetected with is_paused_by_touch + touch_timeout_remaining
- [ ] Replace TimedEventsListed with timed_events_list
- [ ] Replace TimedTapCountdown with timed_tap_countdown
- [ ] Replace DeviceReconnected with device_info + status signals
- [ ] Replace all event_tx.send(AutomationEvent::X) with direct signal.set() calls (23 remaining)
- [ ] Remove event channel imports
- [ ] Update constructor to accept signals instead of event_tx
- [ ] Handle screenshot encoding inline (spawn_blocking for base64_encode)

### Phase 4: Update dioxus_app.rs
- [ ] Remove create_automation_channels() event channel creation
- [ ] Remove event_rx receiver
- [ ] Remove event receiver spawn task (lines ~359-430)
- [ ] Pass signals directly to GameAutomation::new()
- [ ] Fix tokio::spawn to use spawn (line 471)
- [ ] Simplify automation initialization

### Phase 5: Update channels.rs
- [ ] Remove AutomationEvent from create_automation_channels()
- [ ] Return only command channel: (Sender<AutomationCommand>, Receiver<AutomationCommand>)
- [ ] Or delete file entirely if only creating simple mpsc channel

### Phase 6: Update types.rs
- [ ] Remove AutomationEvent enum (no longer needed)
- [ ] Keep AutomationCommand enum (still used for GUI → Backend)

### Phase 7: Testing & Cleanup
- [ ] Test basic connection and screenshot
- [ ] Test automation start/stop
- [ ] Test timed events
- [ ] Test touch monitoring pause/resume
- [ ] Test device disconnect/reconnect
- [ ] Remove unused imports
- [ ] Check for any remaining tokio::spawn usage
- [ ] Run cargo clippy
- [ ] Update documentation

## Files to Modify
1. `src/game_automation/fsm.rs` - Main logic changes
2. `src/game_automation/types.rs` - Remove AutomationEvent
3. `src/game_automation/channels.rs` - Simplify or delete
4. `src/gui/dioxus_app.rs` - Remove event receiver, pass signals
5. `src/game_automation/mod.rs` - Update exports

## Benefits After Refactor
- ✅ Simpler code flow (direct updates)
- ✅ Less overhead (no event serialization/deserialization)
- ✅ Fewer async tasks (remove event receiver)
- ✅ Better Dioxus integration (proper spawn usage)
- ✅ Easier to debug (direct signal flow)
- ✅ Still maintains separation via command channel for GUI → Backend

## Rollback Plan
If issues arise, revert to commit before refactor starts.
