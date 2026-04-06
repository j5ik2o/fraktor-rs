use crate::core::r#impl::queue::BoundedSourceQueue as CoreBoundedSourceQueue;

/// Bounded queue materialized by `Source::queue`.
pub type BoundedSourceQueue<T> = CoreBoundedSourceQueue<T>;
