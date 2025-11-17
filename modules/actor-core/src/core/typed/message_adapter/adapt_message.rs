//! Inline message adaptation for ask/pipe_to_self flows.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::any::TypeId;

use fraktor_utils_core_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::typed::message_adapter::{AdapterEntry, AdapterFailure, AdapterOutcome, AdapterPayload};

/// Represents a one-off adapter invocation scheduled on the parent actor thread.
pub struct AdaptMessage<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  entry:   ArcShared<AdapterEntry<M, TB>>,
  payload: ToolboxMutex<Option<AdapterPayload<TB>>, TB>,
}

impl<M, TB> AdaptMessage<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new inline adapter around the provided value and closure.
  pub fn new<U, F>(value: U, adapter: F) -> Self
  where
    U: Send + Sync + 'static,
    F: Fn(U) -> Result<M, AdapterFailure> + Send + Sync + 'static, {
    let payload = AdapterPayload::new(value);
    let entry = ArcShared::new(AdapterEntry::<M, TB>::new::<U, F>(TypeId::of::<U>(), adapter));
    let storage = <TB::MutexFamily as SyncMutexFamily>::create(Some(payload));
    Self { entry, payload: storage }
  }

  /// Executes the adapter and returns the outcome.
  pub fn execute(&self) -> AdapterOutcome<M> {
    match self.payload.lock().take() {
      | Some(payload) => self.entry.invoke(payload),
      | None => AdapterOutcome::Failure(AdapterFailure::Custom(String::from("payload_consumed"))),
    }
  }
}
