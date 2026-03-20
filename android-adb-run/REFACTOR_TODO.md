# Architecture Refactor TODO (Current)

This file tracks **remaining architecture work** only.
Completed migration items (event-channel → direct signal updates) were removed.

## Snapshot (2026-03)

- Signal/channel hybrid is in place and working.
- Main remaining debt is **module size**, **separation of concerns**, and **legacy compatibility scaffolding**.
- Refactor priorities below are based on current code under `src/`.

## Priority A (do next)

1. [x] Split `src/game_automation/fsm.rs` into focused modules
   - Problem: file is very large and mixes command handling, scheduling, reconnect logic, and screenshot/matching flow.
   - Create `fsm/commands.rs`, `fsm/scheduler.rs`, `fsm/reconnect.rs`, `fsm/run_loop.rs` (or equivalent).
   - Keep `GameAutomation` as orchestrator and preserve public API.

2. [x] Split `src/gui/hooks/device_loop.rs` by responsibility
   - Problem: discovery, reconnect polling, screenshot pipeline, and template matching all live in one hook.
   - Extract: `device_discovery`, `connection_monitor`, `initial_screenshot`, `template_matching_pipeline`.
   - Keep `use_device_loop(...)` as stable entry point.

3. [ ] Move hardcoded timed tap definitions to config
   - Current defaults are embedded in `GameAutomation::new()`.
   - Introduce a loadable config source (file/env) with safe fallback defaults.
   - Ensure interval clamping rules remain enforced.

## Priority B (high value cleanup)

4. [x] Replace tuple device metadata with a struct in GUI layer
   - Today: `type DeviceInfoTuple = (String, Option<u32>, u32, u32)` in `src/gui/hooks/types.rs`.
   - Replace with named fields to reduce index-order mistakes and improve readability.

5. [x] Remove legacy signal type aliases after migration completion
   - `src/gui/hooks/types.rs` still exposes backward-compatibility aliases.
   - Remove aliases not used by current code paths and keep only grouped signal structs.

6. [x] Reconcile build metadata usage in GUI header
   - `src/gui/dioxus_app.rs` uses a placeholder for build year.
   - Consume build-script env vars (`APP_BUILD_YEAR`, `APP_VERSION_DISPLAY`) consistently.

7. [x] Reduce `#[allow(dead_code)]` footprint in ADB/GUI paths
   - Audit unused helpers in `src/adb/usb_impl.rs` and `src/gui/hooks/device_loop.rs`.
   - Either remove dead code or move to explicitly feature-gated modules.

## Priority C (after A/B)

8. [ ] Isolate template matching execution service
   - Create a dedicated service boundary between GUI hooks and matching engine.
   - Goal: clearer ownership for decode/match/result-history logic and easier testing.

9. [ ] Add architecture-level tests around boundaries
   - Focus on: reconnect behavior, timed event scheduler behavior, and device loop phase transitions.
   - Keep tests device-independent where possible.

## Done / removed from backlog

- Event channel removal and direct signal updates.
- `channels.rs` removal and `AutomationEvent` cleanup.
- Basic signal flow wiring between GUI hooks and `GameAutomation`.
