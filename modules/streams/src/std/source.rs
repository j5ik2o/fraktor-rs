extern crate std;

use std::{panic, string::ToString, sync::mpsc, thread};

use crate::core::{
  BoundedSourceQueue, StreamDslError, StreamError, StreamNotUsed, stage::Source, validate_positive_argument,
};

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
      let source = Source::queue(capacity).expect("validated capacity");
      let (graph, queue) = source.into_parts();
      let producer_queue = queue.clone();
      let failure_queue = queue.clone();
      let panic_queue = queue.clone();
      let (started_tx, started_rx) = mpsc::sync_channel(1);

      let spawn_result = thread::Builder::new().name("fraktor-streams-create".to_string()).spawn(move || {
        let _ = started_tx.send(());
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| producer(producer_queue)));
        if result.is_err() {
          let _ = panic_queue.fail_if_open(StreamError::Failed);
        }
      });
      match spawn_result {
        | Ok(_) => {
          let _ = started_rx.recv();
        },
        | Err(_) => {
          let _ = failure_queue.fail_if_open(StreamError::Failed);
        },
      }

      Source::from_graph(graph, StreamNotUsed::new())
    }))
  }
}
