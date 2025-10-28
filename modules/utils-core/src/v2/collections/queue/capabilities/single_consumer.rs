use super::super::TypeKey;

/// Marker trait for queues restricted to a single consumer.
pub trait SingleConsumer: TypeKey {}
