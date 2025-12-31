//! Redelivery tick marker message.

/// Marker message for triggering redelivery.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RedeliveryTick;
