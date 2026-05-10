# Android ADB Automation

A desktop tool for controlling and automating an Android device over USB using the ADB protocol — no ADB daemon required. Primary use-cases include game automation, UI testing, and hands-free device control.

## Language

**Automation**:
The overall orchestration of timed actions executed against a connected device.
_Avoid_: GameAutomation, game loop, bot

**AutomationState**:
The lifecycle state of the Automation engine — `Idle`, `Running`, or `Paused`.
_Avoid_: GameState

### Scheduling

**TimedEvent**:
A recurring automation action (Tap, Swipe, or TextInput) with a fixed interval, an enabled flag, and an execution count. May optionally be linked to one or more Scenes.

Scene-scoping rules:
- **No Scenes set** — fires unconditionally; interval clock runs continuously.
- **One or more Scenes set** — interval clock resets (to one full interval) when transitioning *out* of all matching Scenes. Continues uninterrupted when moving between two Scenes that are both in the set. First fire after Scene activation is after one full interval, not immediately.

_Avoid_: scheduled task, cron, job

### Gestures

**Tap**:
A touch at a single point on the device screen.

**Swipe**:
A drag gesture from one screen point to another, with an optional duration.

**TextInput**:
Typing a string into the currently focused field on the device.
_Avoid_: KeyEvent, SendText, input text

### Device

**Device**:
An Android device reachable over USB. Starts with a minimal identity (transport ID / serial) discovered via ADB enumeration, then gains richer attributes (screen dimensions, display name) after a connection is established.
_Avoid_: DeviceInfo — these are not two separate concepts, just two phases of the same Device lifecycle.

### Human Interaction

**HumanActive**:
The state in which a human is considered to be actively using the device, causing Automation to pause. Determined by whether `HumanLastActivityTimestamp` is within the configured timeout window.

**HumanLastActivityTimestamp**:
The timestamp of the most recent detected human touch on the device. Automation resumes automatically once this timestamp ages past the timeout.
_Avoid_: TouchActivityMonitor, TouchActivityState, last_touch_time

### Screenshot

**Screenshot**:
A live pixel capture from the connected device screen. Used for real-time display in the GUI and as the search image for template matching. A Screenshot saved to disk becomes a **Template** only once a sidecar metadata file defining at least one MatchTarget is added alongside it.

The GUI Screenshot panel operates in two modes:
- **Capture mode** — click/drag gestures are sent to the device (Tap / Swipe). A "Screenshot" action refreshes the display from the live device. "Save img" persists the currently displayed image to `assets/test_images/` under a user-supplied name.
- **TemplateEditor mode** — click/drag gestures draw MatchTarget selection boxes instead of sending device commands. "Load img" opens a saved PNG from `assets/test_images/` (picked from a list) and renders all existing MatchTargets from its sidecar as labelled, selectable overlay boxes.

**TemplateEditor**:
The GUI mode for annotating a saved PNG with MatchTargets. In TemplateEditor mode the user can:
- View all existing MatchTargets for the loaded image as labelled coloured boxes.
- Click a box to select it, then rename or delete it.
- Drag a new selection box and provide a name to create a new MatchTarget.
- All additions and deletions are saved immediately to the sidecar file.

_Avoid_: annotation editor, template builder

### Scenes

**Scene**:
A named classification of what is currently displayed on the device screen (e.g. "login", "admin", "home"). A Scene is defined by the expected MatchTargets that must be visible to confirm it. Detected by analysing a Screenshot.

A Scene can relate to a TimedEvent in two ways:
- **Precondition** — the TimedEvent only fires when the Scene is currently active.
- **Trigger** — a one-off action fires the first time the Scene is detected.

_Avoid_: GameState (reserved for AutomationState), AppState, ScreenState

### Image Recognition

**Template**:
A PNG image file on disk used as the source for image matching. All Templates live in `assets/test_images/`. A PNG file in that directory without a sidecar is a saved Screenshot, not yet a Template.

**MatchTarget**:
A named sub-rectangle within a Template, defined in the Template's sidecar file. This is the actual unit matched against a Screenshot. One Template can define many MatchTargets.

Sidecar file convention: `<image-stem>.json` alongside the PNG. Minimal schema:
```json
{
  "match_targets": [
    { "name": "login_button", "crop": { "x": 120, "y": 540, "width": 200, "height": 60 } }
  ]
}
```
The `crop` rectangle is in Template-image pixel coordinates (not device screen coordinates). Additional fields (SearchRegion, Scene links) may be added per MatchTarget in future without breaking the format.

_Avoid_: patch, region, PatchInfo

**Match**:
The result of finding a MatchTarget on screen — includes the screen coordinates and a confidence score.

**SearchRegion**:
The rectangular area of the screen to search within when looking for a MatchTarget. Limits matching to a sub-area of the Screenshot for performance or precision.
_Avoid_: search area, bounds

## Relationships

- A **Device** provides **Screenshots** and accepts **Gestures** (Tap, Swipe, TextInput)
- A **Screenshot** is the search image for all **MatchTarget** matching
- A **Screenshot** saved to `assets/test_images/` is a **Template** once a sidecar defines its **MatchTargets**
- A **Template** contains one or more **MatchTargets** defined in a `<name>.json` sidecar file
- A **MatchTarget** found in a **Screenshot** produces a **Match** with screen coordinates
- A **Scene** is confirmed when its expected **MatchTargets** are all present in a **Screenshot**
- A **TimedEvent** fires a **Gesture** on an interval, optionally scoped to one or more **Scenes**
- **HumanActive** is true when **HumanLastActivityTimestamp** is within the timeout window, which pauses the **Automation**
- The **TemplateEditor** reads and writes **MatchTargets** from a Template's sidecar; changes are saved immediately

## Example dialogue

> **Dev:** "When the app detects the 'home' Scene, should the collect-reward TimedEvent fire straight away?"
>
> **Domain expert:** "No — it should wait one full interval. The Scene Trigger is for immediate one-off actions on Scene entry. The TimedEvent starts its clock from Scene activation and fires after its interval has elapsed."
>
> **Dev:** "What if the device briefly shows a loading screen between two home-like Scenes?"
>
> **Domain expert:** "If both Scenes are in the TimedEvent's Scene set, the clock keeps running — no reset. Only a transition out of *all* matching Scenes resets it."
>
> **Dev:** "The user touches the phone mid-interval. Does that queued Tap still execute?"
>
> **Domain expert:** "No — HumanActive is true, so the TimedEvent is skipped for that cycle. It's not deferred; it just misses that firing."

## Flagged ambiguities

- "GameAutomation" / "GameState" appear throughout the code but the domain is not game-specific — resolved: the canonical terms are **Automation** and **AutomationState**.
- `DetectionResult.suggested_state` uses `GameState` (Idle/Running/Paused) to represent what's on screen — this conflates engine lifecycle with screen content. Resolved: screen content classification is a **Scene**; `AutomationState` is the engine lifecycle only.
- `Device` and `DeviceInfo` are two structs for one concept — resolved: a single **Device** with discovery and connected phases.
