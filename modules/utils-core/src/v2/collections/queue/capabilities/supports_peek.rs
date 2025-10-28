use super::super::TypeKey;

/// Marker trait for queues that can peek at elements without removal.
pub trait SupportsPeek: TypeKey {}
