// Types and enums for game automation
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum GameState {
    Idle,
    Running, // Simplified from multiple states
    Paused,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimedEventType {
    Screenshot,
    Tap { x: u32, y: u32 },
    CountdownUpdate,
}

#[derive(Debug, Clone)]
pub struct TimedEvent {
    pub id: String,
    pub event_type: TimedEventType,
    pub interval: Duration,
    pub last_executed: Option<Instant>,
    pub enabled: bool,
    pub repeating: bool,
}

// Custom PartialEq implementation since Instant doesn't implement PartialEq
impl PartialEq for TimedEvent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.event_type == other.event_type
            && self.interval == other.interval
            && self.enabled == other.enabled
            && self.repeating == other.repeating
            // Intentionally skip last_executed for comparison since Instant doesn't implement PartialEq
    }
}

impl TimedEvent {
    pub fn new_screenshot(interval_seconds: u64) -> Self {
        Self {
            id: "screenshot".to_string(),
            event_type: TimedEventType::Screenshot,
            interval: Duration::from_secs(interval_seconds),
            last_executed: None,
            enabled: true,
            repeating: true,
        }
    }

    pub fn new_tap(id: String, x: u32, y: u32, interval_minutes: u64) -> Self {
        Self {
            id,
            event_type: TimedEventType::Tap { x, y },
            interval: Duration::from_secs(interval_minutes * 60),
            last_executed: None,
            enabled: true,
            repeating: true,
        }
    }

    pub fn new_countdown_update(interval_seconds: u64) -> Self {
        Self {
            id: "countdown_update".to_string(),
            event_type: TimedEventType::CountdownUpdate,
            interval: Duration::from_secs(interval_seconds),
            last_executed: None,
            enabled: true,
            repeating: true,
        }
    }

    pub fn is_ready(&self) -> bool {
        if !self.enabled {
            return false;
        }

        match self.last_executed {
            None => true, // Never executed, ready to go
            Some(last) => last.elapsed() >= self.interval,
        }
    }

    pub fn mark_executed(&mut self) {
        self.last_executed = Some(Instant::now());
    }

    pub fn time_until_next(&self) -> Option<Duration> {
        if !self.enabled {
            return None;
        }

        match self.last_executed {
            None => Some(Duration::from_secs(0)), // Ready now
            Some(last) => {
                let elapsed = last.elapsed();
                if elapsed >= self.interval {
                    Some(Duration::from_secs(0)) // Ready now
                } else {
                    Some(self.interval - elapsed)
                }
            }
        }
    }

    pub fn get_next_execution_time(&self) -> Option<Instant> {
        if !self.enabled {
            return None;
        }

        match self.last_executed {
            None => Some(Instant::now()),
            Some(last) => Some(last + self.interval),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AutomationCommand {
    Start,
    Pause,
    Resume,
    Stop,
    TakeScreenshot,
    UpdateInterval(u64),       // seconds
    TestImageRecognition,      // Test current screenshot for template matches
    RescanTemplates,           // Rescan directory for new template files
    AddTimedEvent(TimedEvent), // Add a new timed event
    RemoveTimedEvent(String),  // Remove timed event by ID
    EnableTimedEvent(String),  // Enable timed event by ID
    DisableTimedEvent(String), // Disable timed event by ID
    TriggerTimedEvent(String), // Trigger timed event immediately by ID
    ListTimedEvents,           // List all configured timed events
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum AutomationEvent {
    ScreenshotReady(Vec<u8>),
    ScreenshotTaken(Vec<u8>, u64), // Screenshot data and timing in milliseconds
    StateChanged(GameState),
    Error(String),
    IntervalUpdate(u64),
    TemplatesUpdated(Vec<String>),      // List of template files found
    TimedTapExecuted(String, u32, u32), // ID, x, y of executed timed tap (for backward compatibility)
    TimedTapCountdown(String, u64), // ID, seconds until next execution (for backward compatibility)
    TimedEventsListed(Vec<TimedEvent>), // Response to ListTimedEvents command
    TimedEventExecuted(String),     // ID of executed timed event
    NextTimedEvent(String, u64),    // ID, seconds until next event
}
