//! Deadletter package.
//!
//! This module contains undeliverable message handling.

mod dead_letter_entry;
mod dead_letter_impl;
mod dead_letter_reason;
mod dead_letter_shared;

pub use dead_letter_entry::{DeadLetterEntry, DeadLetterEntryGeneric};
pub use dead_letter_impl::{DeadLetter, DeadLetterGeneric};
pub use dead_letter_reason::DeadLetterReason;
pub use dead_letter_shared::{DeadLetterShared, DeadLetterSharedGeneric};

#[cfg(test)]
mod tests;
