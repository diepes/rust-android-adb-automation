// Types and enums for game automation
use std::time::{Duration, Instant};

pub const MIN_TAP_INTERVAL_SECONDS: u64 = 5;
pub const MAX_TAP_INTERVAL_SECONDS: u64 = 6 * 60 * 60; // 6 hours upper bound for GUI adjustments

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
    pub execution_count: u64, // Counter for number of times this event has been executed
}

// Custom PartialEq implementation since Instant doesn't implement PartialEq
impl PartialEq for TimedEvent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.event_type == other.event_type
            && self.interval == other.interval
            && self.enabled == other.enabled
            && self.repeating == other.repeating
            && self.execution_count == other.execution_count
        // Intentionally skip last_executed for comparison since Instant doesn't implement PartialEq
    }
}

impl TimedEvent {
    // Generic constructor that takes Duration directly for maximum flexibility
    pub fn new(id: String, event_type: TimedEventType, interval: Duration) -> Self {
        Self {
            id,
            event_type,
            interval,
            last_executed: None,
            enabled: true,
            repeating: true,
            execution_count: 0,
        }
    }

    pub fn new_screenshot(interval_seconds: u64) -> Self {
        Self {
            id: "screenshot".to_string(),
            event_type: TimedEventType::Screenshot,
            interval: Duration::from_secs(interval_seconds),
            last_executed: None,
            enabled: true,
            repeating: true,
            execution_count: 0,
        }
    }

    pub fn new_screenshot_minutes(interval_minutes: u64) -> Self {
        Self::new_screenshot(interval_minutes * 60)
    }

    pub fn new_tap(id: String, x: u32, y: u32, interval: Duration) -> Self {
        Self {
            id,
            event_type: TimedEventType::Tap { x, y },
            interval,
            last_executed: None,
            enabled: true,
            repeating: true,
            execution_count: 0,
        }
    }

    pub fn new_tap_seconds(id: String, x: u32, y: u32, interval_seconds: u64) -> Self {
        Self::new_tap(id, x, y, Duration::from_secs(interval_seconds))
    }

    pub fn new_tap_minutes(id: String, x: u32, y: u32, interval_minutes: u64) -> Self {
        Self::new_tap(id, x, y, Duration::from_secs(interval_minutes * 60))
    }

    pub fn new_countdown_update(interval_seconds: u64) -> Self {
        Self {
            id: "countdown_update".to_string(),
            event_type: TimedEventType::CountdownUpdate,
            interval: Duration::from_secs(interval_seconds),
            last_executed: None,
            enabled: true,
            repeating: true,
            execution_count: 0,
        }
    }

    pub fn new_countdown_update_minutes(interval_minutes: u64) -> Self {
        Self::new_countdown_update(interval_minutes * 60)
    }

    // Additional convenience methods for common time patterns
    pub fn new_tap_millis(id: String, x: u32, y: u32, interval_millis: u64) -> Self {
        Self::new_tap(id, x, y, Duration::from_millis(interval_millis))
    }

    pub fn new_tap_hours(id: String, x: u32, y: u32, interval_hours: u64) -> Self {
        Self::new_tap(id, x, y, Duration::from_secs(interval_hours * 3600))
    }

    pub fn new_screenshot_millis(interval_millis: u64) -> Self {
        Self::new_screenshot_custom(Duration::from_millis(interval_millis))
    }

    pub fn new_screenshot_hours(interval_hours: u64) -> Self {
        Self::new_screenshot_custom(Duration::from_secs(interval_hours * 3600))
    }

    pub fn new_screenshot_custom(interval: Duration) -> Self {
        Self {
            id: "screenshot".to_string(),
            event_type: TimedEventType::Screenshot,
            interval,
            last_executed: None,
            enabled: true,
            repeating: true,
            execution_count: 0,
        }
    }

    pub fn is_ready(&self) -> bool {
        if !self.enabled {
            return false;
        }

        match self.last_executed {
            None => {
                // Never executed, ready to go
                true
            }
            Some(last) => {
                let elapsed = last.elapsed();
                let ready = elapsed >= self.interval;
                if ready && self.id != "countdown_update" && self.id != "screenshot" {
                    println!("🔔 Event '{}' is ready: elapsed={:?}, interval={:?}", 
                        self.id, elapsed, self.interval);
                }
                ready
            }
        }
    }

    pub fn mark_executed(&mut self) {
        self.last_executed = Some(Instant::now());
        self.execution_count += 1;
    }

    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
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
    TestImageRecognition,      // Test current screenshot for template matches
    RescanTemplates,           // Rescan directory for new template files
    AddTimedEvent(TimedEvent), // Add a new timed event
    RemoveTimedEvent(String),  // Remove timed event by ID
    EnableTimedEvent(String),  // Enable timed event by ID
    DisableTimedEvent(String), // Disable timed event by ID
    TriggerTimedEvent(String), // Trigger timed event immediately by ID
    ListTimedEvents,           // List all configured timed events
    ClearTouchActivity,        // Clear touch activity to resume automation immediately
    RegisterTouchActivity,     // Register touch activity to pause automation for 30 seconds
    AdjustTimedEventInterval { id: String, delta_seconds: i64 }, // Adjust interval for timed tap events
    Shutdown,
}
