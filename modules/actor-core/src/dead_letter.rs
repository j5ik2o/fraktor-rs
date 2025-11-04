//! Deadletter package.
//!
//! This module contains undeliverable message handling.

mod dead_letter_entry;
mod dead_letter_impl;
mod dead_letter_reason;

pub use dead_letter_entry::DeadLetterEntry;
pub use dead_letter_impl::{DeadLetter, DeadLetterGeneric};
pub use dead_letter_reason::DeadLetterReason;

#[cfg(test)]
mod tests;
