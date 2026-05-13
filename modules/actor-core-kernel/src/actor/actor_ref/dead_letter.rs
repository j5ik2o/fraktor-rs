//! Deadletter package.
//!
//! This module contains undeliverable message handling.

mod base;
mod dead_letter_entry;
mod dead_letter_reason;
mod dead_letter_shared;

pub use base::DeadLetter;
pub use dead_letter_entry::DeadLetterEntry;
pub use dead_letter_reason::DeadLetterReason;
pub use dead_letter_shared::DeadLetterShared;

#[cfg(test)]
#[path = "dead_letter_test.rs"]
mod tests;
