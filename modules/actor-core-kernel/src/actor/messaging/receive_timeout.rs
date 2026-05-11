//! Classic receive-timeout auto-message.

#[cfg(test)]
#[path = "receive_timeout_test.rs"]
mod tests;

/// Auto-received message delivered when a configured receive timeout expires.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReceiveTimeout;
