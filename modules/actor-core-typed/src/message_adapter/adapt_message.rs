//! Inline message adaptation for ask/pipe_to_self flows.

#[cfg(test)]
#[path = "adapt_message_test.rs"]
mod tests;

use alloc::string::String;
use core::any::TypeId;

use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::message_adapter::{AdapterEntry, AdapterError, AdapterOutcome, AdapterPayload};

/// Represents a one-off adapter invocation scheduled on the parent actor thread.
pub(crate) struct AdaptMessage<M>
where
  M: Send + Sync + 'static, {
  entry:   ArcShared<AdapterEntry<M>>,
  payload: SharedLock<Option<AdapterPayload>>,
}

impl<M> AdaptMessage<M>
where
  M: Send + Sync + 'static,
{
  /// Creates a new inline adapter around the provided value and closure.
  pub(crate) fn new<U, F>(value: U, adapter: F) -> Self
  where
    U: Send + Sync + 'static,
    F: Fn(U) -> Result<M, AdapterError> + Send + Sync + 'static, {
    let payload = AdapterPayload::new(value);
    let entry = ArcShared::new(AdapterEntry::<M>::new::<U, F>(TypeId::of::<U>(), adapter));
    let storage = SharedLock::new_with_driver::<DefaultMutex<_>>(Some(payload));
    Self { entry, payload: storage }
  }

  /// Executes the adapter and returns the outcome.
  pub(crate) fn execute(&self) -> AdapterOutcome<M> {
    match self.payload.with_lock(|payload| payload.take()) {
      | Some(payload) => self.entry.invoke(payload),
      | None => AdapterOutcome::Failure(AdapterError::Custom(String::from("payload_consumed"))),
    }
  }
}
