extern crate std;

use std::{panic, string::ToString, thread, time::Duration};

use crate::core::{
  BoundedSourceQueue, OverflowStrategy, StreamDslError, StreamError, StreamNotUsed, stage::Source,
  validate_positive_argument,
};

const CREATE_SOURCE_POLL_INTERVAL: Duration = Duration::from_millis(1);

enum CreateSourceEvent<T> {
  Item(T),
  Failed(StreamError),
}

struct CreateSourceIterator<T> {
  queue:           BoundedSourceQueue<T>,
  emitted_failure: bool,
}

impl<T> CreateSourceIterator<T> {
  const fn new(queue: BoundedSourceQueue<T>) -> Self {
    Self { queue, emitted_failure: false }
  }
}

impl<T> Iterator for CreateSourceIterator<T> {
  type Item = CreateSourceEvent<T>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.emitted_failure {
      return None;
    }
    loop {
      match self.queue.poll() {
        | Ok(Some(value)) => return Some(CreateSourceEvent::Item(value)),
        | Ok(None) if self.queue.is_drained() => return None,
        | Ok(None) => thread::sleep(CREATE_SOURCE_POLL_INTERVAL),
        | Err(error) => {
          self.emitted_failure = true;
          return Some(CreateSourceEvent::Failed(error));
        },
      }
    }
  }
}

impl<T> Drop for CreateSourceIterator<T> {
  fn drop(&mut self) {
    if !self.queue.is_closed() {
      self.queue.complete();
    }
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
  pub fn create<F>(capacity: usize, producer: F) -> Result<Source<Out, StreamNotUsed>, StreamDslError>
  where
    F: FnOnce(BoundedSourceQueue<Out>) + Send + 'static, {
    let capacity = validate_positive_argument("capacity", capacity)?;
    Ok(Self::lazy_source(move || {
      let queue = BoundedSourceQueue::new(capacity, OverflowStrategy::Backpressure);
      let producer_queue = queue.clone();
      let termination_queue = queue.clone();

      let spawn_result = thread::Builder::new().name("fraktor-streams-create".to_string()).spawn(move || {
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| producer(producer_queue)));
        match result {
          | Ok(()) => {
            if !termination_queue.is_closed() {
              termination_queue.complete();
            }
          },
          | Err(_) => {
            let _ = termination_queue.fail_if_open(StreamError::Failed);
          },
        }
      });

      if spawn_result.is_err() {
        let _ = queue.fail_if_open(StreamError::Failed);
        return Source::failed(StreamError::Failed);
      }

      Source::from_iterator(CreateSourceIterator::new(queue)).flat_map_concat(|event| match event {
        | CreateSourceEvent::Item(value) => Source::single(value),
        | CreateSourceEvent::Failed(error) => Source::failed(error),
      })
    }))
  }
}
