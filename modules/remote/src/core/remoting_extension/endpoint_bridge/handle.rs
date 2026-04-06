//! Opaque handle trait for a running endpoint transport bridge.

/// Opaque handle for a running endpoint transport bridge.
///
/// `core` only needs to keep the handle alive (drop = shutdown). Adapter
/// implementations may expose additional methods on their concrete types.
pub trait EndpointBridgeHandle: Send + Sync {}
