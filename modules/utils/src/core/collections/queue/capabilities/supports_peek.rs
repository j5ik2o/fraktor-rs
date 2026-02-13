use crate::core::collections::queue::type_keys::TypeKey;

/// Marker trait for queues that can peek at elements without removal.
pub trait SupportsPeek: TypeKey {}
