/// DeadLetter store specialised for `StdToolbox` (shared wrapper).
pub type DeadLetter = crate::core::dead_letter::DeadLetterShared;

/// Captures a single deadletter occurrence.
pub type DeadLetterEntry = crate::core::dead_letter::DeadLetterEntry;
