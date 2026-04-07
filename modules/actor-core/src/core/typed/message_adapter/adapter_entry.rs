//! Internal registry entry describing an adapter function.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String};
use core::any::TypeId;

use fraktor_utils_core_rs::core::sync::shared::Shared;

use crate::core::typed::message_adapter::{AdapterError, AdapterOutcome, AdapterPayload};

/// Stores adapter metadata and execution closure.
pub(crate) struct AdapterEntry<M>
where
  M: Send + Sync + 'static, {
  type_id: TypeId,
  handler: Box<dyn Fn(AdapterPayload) -> AdapterOutcome<M> + Send + Sync + 'static>,
}

impl<M> AdapterEntry<M>
where
  M: Send + Sync + 'static,
{
  /// Creates a new registry entry placeholder.
  #[must_use]
  pub(crate) fn new<U, F>(type_id: TypeId, adapter: F) -> Self
  where
    U: Send + Sync + 'static,
    F: Fn(U) -> Result<M, AdapterError> + Send + Sync + 'static, {
    let handler = Box::new(move |payload: AdapterPayload| match payload.try_downcast::<U>() {
      | Ok(value) => match value.try_unwrap() {
        | Ok(concrete) => match adapter(concrete) {
          | Ok(result) => AdapterOutcome::Converted(result),
          | Err(failure) => AdapterOutcome::Failure(failure),
        },
        | Err(_) => AdapterOutcome::Failure(AdapterError::Custom(String::from("payload_shared"))),
      },
      | Err(_) => AdapterOutcome::Failure(AdapterError::TypeMismatch(type_id)),
    });
    Self { type_id, handler }
  }

  /// Returns the payload [`TypeId`] matched by this entry.
  #[must_use]
  pub(crate) const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Executes the adapter closure.
  #[must_use]
  pub(crate) fn invoke(&self, payload: AdapterPayload) -> AdapterOutcome<M> {
    (self.handler)(payload)
  }
}
