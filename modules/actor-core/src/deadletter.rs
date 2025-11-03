//! Deadletter package.
//!
//! This module contains undeliverable message handling.

mod deadletter_entry;
mod deadletter_impl;
mod deadletter_reason;

pub use deadletter_entry::DeadletterEntry;
pub use deadletter_impl::{Deadletter, DeadletterGeneric};
pub use deadletter_reason::DeadletterReason;

#[cfg(test)]
mod tests;
