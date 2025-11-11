//! Internal registry entry describing an adapter function.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String};
use core::any::TypeId;

use fraktor_utils_core_rs::Shared;

use crate::{
  RuntimeToolbox,
  typed::message_adapter::{AdapterFailure, AdapterOutcome, AdapterPayload},
};

/// Stores adapter metadata and execution closure.
pub struct AdapterEntry<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  type_id: TypeId,
  handler: Box<dyn Fn(AdapterPayload<TB>) -> AdapterOutcome<M> + Send + Sync + 'static>,
}

impl<M, TB> AdapterEntry<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new registry entry placeholder.
  #[must_use]
  pub fn new<U, F>(type_id: TypeId, adapter: F) -> Self
  where
    U: Send + Sync + 'static,
    F: Fn(U) -> Result<M, AdapterFailure> + Send + Sync + 'static, {
    let handler = Box::new(move |payload: AdapterPayload<TB>| match payload.try_downcast::<U>() {
      | Ok(value) => match value.try_unwrap() {
        | Ok(concrete) => match adapter(concrete) {
          | Ok(result) => AdapterOutcome::Converted(result),
          | Err(failure) => AdapterOutcome::Failure(failure),
        },
        | Err(_) => AdapterOutcome::Failure(AdapterFailure::Custom(String::from("payload_shared"))),
      },
      | Err(_) => AdapterOutcome::Failure(AdapterFailure::TypeMismatch(type_id)),
    });
    Self { type_id, handler }
  }

  /// Returns the payload [`TypeId`] matched by this entry.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Executes the adapter closure.
  #[must_use]
  pub fn invoke(&self, payload: AdapterPayload<TB>) -> AdapterOutcome<M> {
    (self.handler)(payload)
  }
}
