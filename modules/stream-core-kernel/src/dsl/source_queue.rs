use crate::r#impl::queue::SourceQueue as CoreSourceQueue;

/// Materialized queue handle for pushing elements into a Source.
pub type SourceQueue<T> = CoreSourceQueue<T>;
