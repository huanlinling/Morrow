//! Opaque handle system for the Ferrum FFI boundary.
//!
//! All Rust objects exposed to Java are referenced through opaque
//! `u64` handles. This module provides the handle-to-object mapping
//! so Java never sees a raw pointer or Rust struct layout.
//!
//! ## Design
//!
//! - Handles are monotonically increasing `u64` values (not pointers).
//! - Each handle maps to exactly one object in an internal registry.
//! - Freeing a handle removes the object and invalidates the handle.
//! - Using a freed handle returns an error, never UB.
//!
//! ## Safety
//!
//! Handle generation starts at 1. A value of `0` always means "no handle"
//! or "error" — this is checked at the FFI boundary in `lib.rs`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Global handle counter — starts at 1 so 0 is always "invalid".
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

// ---------------------------------------------------------------------------
// Handle type
// ---------------------------------------------------------------------------

/// An opaque handle that Java sees as a `u64`.
///
/// `Handle(0)` represents an invalid/null handle. All valid handles
/// are non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Handle(u64);

impl Handle {
    /// Allocate a new unique handle.
    pub fn new() -> Self {
        Handle(NEXT_ID.fetch_add(1, Ordering::SeqCst))
    }

    /// Return the raw u64 for passing across FFI.
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Reconstruct a Handle from a u64 received across FFI.
    ///
    /// Returns `None` if the raw value is 0 (invalid).
    pub fn from_u64(raw: u64) -> Option<Self> {
        if raw == 0 {
            None
        } else {
            Some(Handle(raw))
        }
    }

    /// Returns true if this is a valid (non-zero) handle.
    #[allow(dead_code)]
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

// ---------------------------------------------------------------------------
// Handle table
// ---------------------------------------------------------------------------

/// A thread-safe table mapping `Handle → T`.
///
/// Used as the internal registry for runtime objects, mod instances, etc.
pub struct HandleTable<T: Send + 'static> {
    entries: Mutex<HashMap<u64, T>>,
}

impl<T: Send + 'static> HandleTable<T> {
    pub fn new() -> Self {
        HandleTable {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Insert an object and return its handle.
    pub fn insert(&self, value: T) -> Handle {
        let handle = Handle::new();
        self.entries.lock().unwrap().insert(handle.0, value);
        handle
    }

    /// Remove an object by handle, returning it if it existed.
    pub fn remove(&self, handle: Handle) -> Option<T> {
        self.entries.lock().unwrap().remove(&handle.0)
    }

    /// Get a reference to the object behind a handle.
    #[allow(dead_code)] // used in M3+ for mod registry lookups
    pub fn get(&self, handle: Handle) -> Option<T>
    where
        T: Clone,
    {
        self.entries.lock().unwrap().get(&handle.0).cloned()
    }

    /// Return the number of live entries.
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    /// True when no handles are allocated.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Send + 'static> Default for HandleTable<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_zero_is_invalid() {
        assert!(Handle::from_u64(0).is_none());
    }

    #[test]
    fn test_handle_nonzero_is_valid() {
        let h = Handle::from_u64(42).unwrap();
        assert_eq!(h.as_u64(), 42);
    }

    #[test]
    fn test_handle_new_is_unique() {
        let a = Handle::new();
        let b = Handle::new();
        assert_ne!(a.as_u64(), b.as_u64());
    }

    #[test]
    fn test_table_insert_remove() {
        let table = HandleTable::new();
        let h = table.insert("hello".to_string());
        assert_eq!(table.len(), 1);
        assert_eq!(table.get(h), Some("hello".to_string()));

        let removed = table.remove(h);
        assert_eq!(removed, Some("hello".to_string()));
        assert_eq!(table.len(), 0);
        assert_eq!(table.get(h), None);
    }

    #[test]
    fn test_table_leak_detection() {
        let table = HandleTable::<String>::new();
        for _ in 0..10 {
            let h = table.insert("test".into());
            table.remove(h);
        }
        assert_eq!(table.len(), 0);
    }
}
