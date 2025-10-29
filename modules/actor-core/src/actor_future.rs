//! Cooperative future primitive used for ask pattern completion.

mod error;

use cellactor_utils_core_rs::{ArcShared, sync::async_mutex_like::SpinAsyncMutex};
pub use error::ActorFutureError;

type ActorFutureCallback<T> = ArcShared<dyn Fn(&T) + Send + Sync + 'static>;

/// Cooperative future handle resolved by the actor runtime.
pub struct ActorFuture<T> {
  inner: SpinAsyncMutex<ActorFutureInner<T>>,
}

impl<T> ActorFuture<T> {
  /// Creates a new pending future.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: SpinAsyncMutex::new(ActorFutureInner { result: None, callback: None }) }
  }

  /// Resolves the future with the provided value.
  pub fn complete(&self, value: T) -> Result<(), ActorFutureError> {
    let mut guard = self.inner.lock();
    if guard.result.is_some() {
      return Err(ActorFutureError::AlreadyCompleted);
    }

    guard.result = Some(value);
    if let Some(callback) = &guard.callback {
      if let Some(result) = guard.result.as_ref() {
        callback(result);
      }
    }
    Ok(())
  }

  /// Returns `true` when the future completed successfully.
  #[must_use]
  pub fn is_completed(&self) -> bool {
    self.inner.lock().result.is_some()
  }

  /// Takes the resolved value if present.
  pub fn take(&self) -> Option<T> {
    self.inner.lock().result.take()
  }

  /// Registers a callback invoked when the future completes.
  pub fn on_complete(&self, callback: ActorFutureCallback<T>) -> Result<(), ActorFutureError> {
    let mut guard = self.inner.lock();

    if guard.callback.is_some() {
      return Err(ActorFutureError::CallbackAlreadyRegistered);
    }

    if let Some(result) = guard.result.as_ref() {
      callback(result);
      return Ok(());
    }

    guard.callback = Some(callback);
    Ok(())
  }
}

struct ActorFutureInner<T> {
  result:   Option<T>,
  callback: Option<ActorFutureCallback<T>>,
}
