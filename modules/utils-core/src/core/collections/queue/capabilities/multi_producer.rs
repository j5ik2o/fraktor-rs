use super::super::TypeKey;

/// Marker trait for queues supporting multiple producers.
pub trait MultiProducer: TypeKey {}
