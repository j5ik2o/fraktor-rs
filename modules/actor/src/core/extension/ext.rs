//! Actor system extension trait.

/// Marker trait implemented by every actor-system extension.
pub trait Extension: Send + Sync + 'static {}
