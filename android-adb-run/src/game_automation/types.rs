// Types and enums for game automation
#[derive(Debug, Clone, PartialEq)]
pub enum GameState {
    Idle,
    WaitingForScreenshot,
    Analyzing,
    Acting,
    Paused,
}

#[derive(Debug, Clone)]
pub enum AutomationCommand {
    Start,
    Pause,
    Resume,
    Stop,
    TakeScreenshot,
    UpdateInterval(u64), // seconds
    TestImageRecognition, // Test current screenshot for template matches
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum AutomationEvent {
    ScreenshotReady(Vec<u8>),
    StateChanged(GameState),
    Error(String),
    IntervalUpdate(u64),
}
