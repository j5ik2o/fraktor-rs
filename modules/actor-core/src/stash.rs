//! Stash-related helper types.

/// Deque handle primitives used by stash-compatible actors.
pub mod deque_handle;

pub use deque_handle::{DequeHandle, StashDequeHandle, StashDequeHandleGeneric};

#[cfg(test)]
mod tests;
