//! Inline message adaptation for ask/pipe_to_self flows.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::any::TypeId;

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::typed::message_adapter::{AdapterEntry, AdapterError, AdapterOutcome, AdapterPayload};

/// Represents a one-off adapter invocation scheduled on the parent actor thread.
pub(crate) struct AdaptMessage<M>
where
  M: Send + Sync + 'static, {
  entry:   ArcShared<AdapterEntry<M>>,
  payload: RuntimeMutex<Option<AdapterPayload>>,
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
    let storage = RuntimeMutex::new(Some(payload));
    Self { entry, payload: storage }
  }

  /// Executes the adapter and returns the outcome.
  pub(crate) fn execute(&self) -> AdapterOutcome<M> {
    match self.payload.lock().take() {
      | Some(payload) => self.entry.invoke(payload),
      | None => AdapterOutcome::Failure(AdapterError::Custom(String::from("payload_consumed"))),
    }
  }
}
