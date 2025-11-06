//! Typed wrapper over ask futures.

use core::marker::PhantomData;

use cellactor_utils_core_rs::{Shared, sync::ArcShared};

use crate::{
  RuntimeToolbox, futures::ActorFuture, messaging::AnyMessageGeneric, typed::typed_ask_error::TypedAskError,
};

/// Exposes typed helpers around an ask future that resolves with `R`.
pub struct TypedAskFuture<R, TB>
where
  R: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  inner:  ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>,
  marker: PhantomData<R>,
}

impl<R, TB> Clone for TypedAskFuture<R, TB>
where
  R: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), marker: PhantomData }
  }
}

impl<R, TB> TypedAskFuture<R, TB>
where
  R: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  pub(crate) const fn new(inner: ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>) -> Self {
    Self { inner, marker: PhantomData }
  }

  /// Returns whether the underlying future has resolved.
  #[must_use]
  pub fn is_ready(&self) -> bool {
    self.inner.is_ready()
  }

  /// Attempts to take the reply if ready, yielding either the typed payload or an error.
  #[must_use]
  pub fn try_take(&self) -> Option<Result<R, TypedAskError>> {
    self.inner.try_take().map(Self::map_message)
  }

  #[allow(clippy::needless_pass_by_value)]
  fn map_message(message: AnyMessageGeneric<TB>) -> Result<R, TypedAskError> {
    let payload = message.payload_arc();
    drop(message);
    match payload.downcast::<R>() {
      | Ok(concrete) => concrete.try_unwrap().map_err(|_| TypedAskError::SharedReferences),
      | Err(_original) => Err(TypedAskError::TypeMismatch),
    }
  }
}
