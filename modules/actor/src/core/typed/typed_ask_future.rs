//! Typed wrapper over ask futures.

use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::{SharedAccess, shared::Shared},
};

use crate::core::{futures::ActorFutureSharedGeneric, messaging::AskResult, typed::typed_ask_error::TypedAskError};

/// Exposes typed helpers around an ask future that resolves with `R`.
pub struct TypedAskFutureGeneric<R, TB>
where
  R: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  inner:  ActorFutureSharedGeneric<AskResult<TB>, TB>,
  marker: PhantomData<R>,
}

/// Type alias with the default toolbox.
pub type TypedAskFuture<R> = TypedAskFutureGeneric<R, NoStdToolbox>;

impl<R, TB> Clone for TypedAskFutureGeneric<R, TB>
where
  R: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), marker: PhantomData }
  }
}

impl<R, TB> TypedAskFutureGeneric<R, TB>
where
  R: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  pub(crate) const fn new(inner: ActorFutureSharedGeneric<AskResult<TB>, TB>) -> Self {
    Self { inner, marker: PhantomData }
  }

  /// Returns whether the underlying future has resolved.
  #[must_use]
  pub fn is_ready(&self) -> bool {
    self.inner.with_read(|af| af.is_ready())
  }

  /// Attempts to take the reply if ready, yielding either the typed payload or an error.
  #[must_use]
  pub fn try_take(&mut self) -> Option<Result<R, TypedAskError>> {
    self.inner.with_write(|af| af.try_take().map(Self::map_result))
  }

  fn map_result(result: AskResult<TB>) -> Result<R, TypedAskError> {
    match result {
      | Ok(message) => Self::map_message(message),
      | Err(ask_error) => Err(TypedAskError::AskFailed(ask_error)),
    }
  }

  #[allow(clippy::needless_pass_by_value)]
  fn map_message(message: crate::core::messaging::AnyMessageGeneric<TB>) -> Result<R, TypedAskError> {
    let payload = message.payload_arc();
    drop(message);
    match payload.downcast::<R>() {
      | Ok(concrete) => concrete.try_unwrap().map_err(|_| TypedAskError::SharedReferences),
      | Err(_original) => Err(TypedAskError::TypeMismatch),
    }
  }
}
