//! Authentication marker for durable state signals.

/// Marker that prevents external crates from forging durable state signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DurableStateSignalAuth(pub(crate) ());
