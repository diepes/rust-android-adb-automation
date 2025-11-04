// Communication channels for game automation
use super::types::{AutomationCommand, AutomationEvent};
use tokio::sync::mpsc;

/// Helper function to create automation channels
pub fn create_automation_channels() -> (
    mpsc::Sender<AutomationCommand>,
    mpsc::Receiver<AutomationCommand>,
    mpsc::Sender<AutomationEvent>,
    mpsc::Receiver<AutomationEvent>,
) {
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    let (event_tx, event_rx) = mpsc::channel(32);
    (cmd_tx, cmd_rx, event_tx, event_rx)
}
