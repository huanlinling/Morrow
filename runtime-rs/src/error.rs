//! Error channel — in-band error reporting across the FFI boundary.
//!
//! Errors are stored per-runtime in a queue. Java polls via:
//! - `morrow_last_error(runtime_handle)` → error_handle (0 = no error)
//! - `morrow_error_message(error_handle, buffer, buffer_cap)` → bytes written
//!
//! See docs/02-abi-design.md § "Error Channel".

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Error record
// ---------------------------------------------------------------------------

static NEXT_ERROR_ID: AtomicU64 = AtomicU64::new(1);

/// A single error record in the channel.
#[derive(Debug, Clone)]
pub struct ErrorRecord {
    pub id: u64,
    pub message: String,
    #[allow(dead_code)]
    pub timestamp: std::time::Instant,
}

/// Thread-safe queue of error records.
pub struct ErrorChannel {
    queue: Mutex<VecDeque<ErrorRecord>>,
}

impl ErrorChannel {
    pub fn new() -> Self {
        ErrorChannel {
            queue: Mutex::new(VecDeque::new()),
        }
    }

    /// Push an error to the channel. Returns the error handle.
    pub fn push(&self, message: impl Into<String>) -> u64 {
        let id = NEXT_ERROR_ID.fetch_add(1, Ordering::SeqCst);
        let record = ErrorRecord {
            id,
            message: message.into(),
            timestamp: std::time::Instant::now(),
        };
        self.queue.lock().unwrap().push_back(record);
        id
    }

    /// Peek at the oldest error (without removing it).
    pub fn peek(&self) -> Option<ErrorRecord> {
        self.queue.lock().unwrap().front().cloned()
    }

    /// Remove and return an error by handle.
    pub fn take(&self, handle: u64) -> Option<ErrorRecord> {
        let mut q = self.queue.lock().unwrap();
        if let Some(pos) = q.iter().position(|e| e.id == handle) {
            q.remove(pos)
        } else {
            None
        }
    }

    /// Number of pending errors.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }
}

impl Default for ErrorChannel {
    fn default() -> Self {
        Self::new()
    }
}
