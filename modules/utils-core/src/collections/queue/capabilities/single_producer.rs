use super::super::TypeKey;

/// Marker trait for queues restricted to a single producer.
pub trait SingleProducer: TypeKey {}
