use crate::core::r#impl::queue::SourceQueueWithComplete as CoreSourceQueueWithComplete;

/// Materialized queue handle with explicit completion for a Source.
pub type SourceQueueWithComplete<T> = CoreSourceQueueWithComplete<T>;
