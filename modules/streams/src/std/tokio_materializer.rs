//! Tokio-based materializer implementation.

#[cfg(test)]
mod tests;

extern crate std;

use std::{
  sync::{Arc, Mutex},
  time::Duration,
  vec::Vec,
};

use crate::{
  core::{Materialized, Materializer, RunnableGraph, StreamError, StreamHandleState, StreamMaterializer},
  std::stream_handle_shared::StreamHandleShared,
};

/// Tokio-backed materializer that drives stream handles periodically.
#[derive(Debug)]
pub struct TokioMaterializer {
  inner:      StreamMaterializer,
  handles:    Arc<Mutex<Vec<Arc<Mutex<StreamHandleState>>>>>,
  drive_task: Option<tokio::task::JoinHandle<()>>,
  tick:       Duration,
}

impl TokioMaterializer {
  /// Creates a new Tokio materializer with a drive interval.
  #[must_use]
  pub fn new(tick: Duration) -> Self {
    Self { inner: StreamMaterializer::new(), handles: Arc::new(Mutex::new(Vec::new())), drive_task: None, tick }
  }

  fn ensure_runtime() -> Result<(), StreamError> {
    tokio::runtime::Handle::try_current().map(|_| ()).map_err(|_| StreamError::ExecutorUnavailable)
  }

  fn spawn_driver(&mut self) {
    let handles = self.handles.clone();
    let tick = self.tick;
    self.drive_task = Some(tokio::spawn(async move {
      loop {
        let snapshot = {
          let guard = match handles.lock() {
            | Ok(guard) => guard,
            | Err(poisoned) => poisoned.into_inner(),
          };
          guard.clone()
        };
        for handle in snapshot {
          let mut guard = match handle.lock() {
            | Ok(guard) => guard,
            | Err(poisoned) => poisoned.into_inner(),
          };
          let _ = guard.drive();
        }
        tokio::time::sleep(tick).await;
      }
    }));
  }
}

impl Materializer for TokioMaterializer {
  type Handle = StreamHandleShared;

  fn start(&mut self) -> Result<(), StreamError> {
    Self::ensure_runtime()?;
    self.inner.start()?;
    self.spawn_driver();
    Ok(())
  }

  fn materialize(&mut self, graph: RunnableGraph) -> Result<Materialized<Self::Handle>, StreamError> {
    Self::ensure_runtime()?;
    let materialized = self.inner.materialize(graph)?;
    let value = materialized.value();
    let handle = Arc::new(Mutex::new(materialized.into_handle()));
    let shared = StreamHandleShared::new(handle.clone());

    let mut guard = match self.handles.lock() {
      | Ok(guard) => guard,
      | Err(poisoned) => poisoned.into_inner(),
    };
    guard.push(handle);

    Ok(Materialized::new(shared, value))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    if let Some(task) = self.drive_task.take() {
      task.abort();
    }
    self.inner.shutdown()
  }
}
