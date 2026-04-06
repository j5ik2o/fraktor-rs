use crate::core::r#impl::queue::SinkQueue as CoreSinkQueue;

/// Materialized queue handle for reading from a Sink.
pub type SinkQueue<T> = CoreSinkQueue<T>;
