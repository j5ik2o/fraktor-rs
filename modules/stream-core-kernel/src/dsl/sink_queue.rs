use crate::r#impl::queue::SinkQueue as CoreSinkQueue;

#[cfg(test)]
mod tests;

/// Materialized queue handle for reading from a Sink.
pub type SinkQueue<T> = CoreSinkQueue<T>;

/// Cancellable materialized queue handle for reading from a Sink.
///
/// Pekko parity: `pekko.stream.scaladsl.SinkQueueWithCancel[T]`. In Pekko this
/// is a sub-trait of `SinkQueue[T]` that adds `cancel()`. The fraktor-rs
/// [`SinkQueue`] already exposes `cancel()` because Rust does not need a
/// separate marker trait for that capability, so `SinkQueueWithCancel<T>` is a
/// transparent type alias rather than a distinct type. Use this name when
/// porting code that referenced `SinkQueueWithCancel` in Pekko Scala/Java DSLs.
pub type SinkQueueWithCancel<T> = CoreSinkQueue<T>;
