use crate::{BoundedSourceQueue, QueueOfferResult, StreamError};

#[cfg(test)]
mod tests;

/// Handle for sending elements into an actor-sourced stream.
///
/// This is the materialized value of [`ActorSource::actor_ref`] and
/// [`ActorSource::actor_ref_with_backpressure`]. It wraps a
/// [`BoundedSourceQueue`] and provides an actor-oriented API
/// (`tell` / `complete` / `fail`) analogous to Pekko's `ActorRef`
/// materialized by `Source.actorRef`.
pub struct ActorSourceRef<T> {
  queue: BoundedSourceQueue<T>,
}

impl<T> Clone for ActorSourceRef<T> {
  fn clone(&self) -> Self {
    Self { queue: self.queue.clone() }
  }
}

impl<T> ActorSourceRef<T> {
  /// Creates a new handle wrapping the given queue.
  #[must_use]
  pub const fn new(queue: BoundedSourceQueue<T>) -> Self {
    Self { queue }
  }

  /// Sends a value into the stream.
  ///
  /// Returns [`QueueOfferResult`] indicating whether the value was
  /// enqueued, dropped, or rejected.
  #[must_use]
  pub fn tell(&mut self, msg: T) -> QueueOfferResult {
    self.queue.offer(msg)
  }

  /// Completes the stream normally.
  ///
  /// # Panics
  ///
  /// Panics when the queue has already been completed or failed.
  pub fn complete(&mut self) {
    self.queue.complete();
  }

  /// Fails the stream with an error.
  ///
  /// # Panics
  ///
  /// Panics when the queue has already been completed or failed.
  pub fn fail(&mut self, error: StreamError) {
    self.queue.fail(error);
  }

  /// Returns `true` when the stream has been completed or failed.
  #[must_use]
  pub fn is_closed(&self) -> bool {
    self.queue.is_closed()
  }
}
