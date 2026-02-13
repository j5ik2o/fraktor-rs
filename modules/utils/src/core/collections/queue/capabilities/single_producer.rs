use crate::core::collections::queue::type_keys::TypeKey;

/// Marker trait for queues restricted to a single producer.
pub trait SingleProducer: TypeKey {}
