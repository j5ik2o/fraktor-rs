extern crate std;

use alloc::boxed::Box;
use std::{panic, string::ToString, thread};

use crate::core::{
  DynValue, OverflowStrategy, SourceLogic, StreamDslError, StreamError, StreamNotUsed,
  queue::BoundedSourceQueue,
  stage::{Source, StageKind},
  validate_positive_argument,
};

#[cfg(test)]
mod tests;

struct CreateSourceLogic<T, F> {
  queue:    BoundedSourceQueue<T>,
  producer: Option<F>,
}

impl<T, F> CreateSourceLogic<T, F> {
  const fn new(queue: BoundedSourceQueue<T>, producer: F) -> Self {
    Self { queue, producer: Some(producer) }
  }

  fn start_producer_if_needed(&mut self) -> Result<(), StreamError>
  where
    T: Send + Sync + 'static,
    F: FnOnce(BoundedSourceQueue<T>) + Send + 'static, {
    let Some(producer) = self.producer.take() else {
      return Ok(());
    };

    let producer_queue = self.queue.clone();
    let termination_queue = self.queue.clone();
    let spawn_result = thread::Builder::new().name("fraktor-streams-create".to_string()).spawn(move || {
      let result = panic::catch_unwind(panic::AssertUnwindSafe(|| producer(producer_queue)));
      match result {
        | Ok(()) => {
          let _ = termination_queue.complete_if_open();
        },
        | Err(_) => {
          let _ = termination_queue.fail_if_open(StreamError::Failed);
        },
      }
    });

    if spawn_result.is_err() {
      let _ = self.queue.fail_if_open(StreamError::Failed);
      return Err(StreamError::Failed);
    }

    Ok(())
  }
}

impl<T, F> SourceLogic for CreateSourceLogic<T, F>
where
  T: Send + Sync + 'static,
  F: FnOnce(BoundedSourceQueue<T>) + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    self.start_producer_if_needed()?;
    // poll_or_drain は 1 回のロック内で poll + drained チェックを行い、
    // TOCTOU レースを防ぐ。
    match self.queue.poll_or_drain()? {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Ok(None),
    }
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    self.queue.close_for_cancel();
    Ok(())
  }
}

impl<Out> Source<Out, StreamNotUsed>
where
  Out: Send + Sync + 'static,
{
  /// Creates a source backed by a bounded source queue and runs the producer asynchronously.
  ///
  /// The producer starts lazily when the source is materialized, but it is executed on a
  /// background thread instead of the stream pull path.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `capacity` is zero.
  ///
  /// # Panics
  ///
  /// Panics if the internally created queue cannot be constructed after
  /// `capacity` has already been validated.
  pub fn create<F>(capacity: usize, producer: F) -> Result<Source<Out, BoundedSourceQueue<Out>>, StreamDslError>
  where
    F: FnOnce(BoundedSourceQueue<Out>) + Send + 'static, {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let queue = BoundedSourceQueue::new(capacity, OverflowStrategy::Backpressure);
    let logic = CreateSourceLogic::new(queue.clone(), producer);
    Ok(Source::from_logic(StageKind::Custom, logic).map_materialized_value(move |_| queue))
  }
}
