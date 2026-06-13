//! Flush-changes protocol vocabulary.

/// Command requesting immediate delivery of pending subscriber notifications.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlushChanges;
