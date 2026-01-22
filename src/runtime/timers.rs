use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Timer ID type
pub type TimerId = u64;

/// Message to execute a timer callback
pub enum TimerMessage {
    /// Execute timeout callback (one-shot)
    Timeout(TimerId),
    /// Execute interval callback (repeating)
    Interval(TimerId),
}

/// Timer manager that handles setTimeout/setInterval
pub struct TimerManager {
    next_id: AtomicU64,
    handles: std::sync::Mutex<HashMap<TimerId, JoinHandle<()>>>,
    tx: mpsc::UnboundedSender<TimerMessage>,
}

impl TimerManager {
    /// Create a new timer manager
    pub fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<TimerMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();

        let manager = Arc::new(Self {
            next_id: AtomicU64::new(1),
            handles: std::sync::Mutex::new(HashMap::new()),
            tx,
        });

        (manager, rx)
    }

    /// Schedule a timeout (one-shot)
    pub fn set_timeout(&self, delay_ms: u64) -> TimerId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let tx = self.tx.clone();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            let _ = tx.send(TimerMessage::Timeout(id));
        });

        self.handles.lock().unwrap().insert(id, handle);
        id
    }

    /// Schedule an interval (repeating)
    pub fn set_interval(&self, interval_ms: u64) -> TimerId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let tx = self.tx.clone();

        let handle = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));
            // Skip the first tick (fires immediately)
            interval.tick().await;

            loop {
                interval.tick().await;

                if tx.send(TimerMessage::Interval(id)).is_err() {
                    break;
                }
            }
        });

        self.handles.lock().unwrap().insert(id, handle);
        id
    }

    /// Clear a timer (timeout or interval)
    pub fn clear_timer(&self, id: TimerId) {
        if let Some(handle) = self.handles.lock().unwrap().remove(&id) {
            handle.abort();
        }
    }
}
