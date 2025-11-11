//! Adapter payload wrapper for erased message values.

#[cfg(test)]
mod tests;

use core::any::TypeId;

use fraktor_utils_core_rs::sync::{ArcShared, NoStdToolbox};

use crate::RuntimeToolbox;

/// Owns a dynamically typed payload destined for message adapters.
#[derive(Debug)]
pub struct AdapterPayload<TB: RuntimeToolbox = NoStdToolbox> {
  inner:   ArcShared<dyn core::any::Any + Send + Sync + 'static>,
  _marker: core::marker::PhantomData<TB>,
}

impl<TB: RuntimeToolbox> AdapterPayload<TB> {
  /// Creates a payload from the provided message.
  #[must_use]
  pub fn new<T>(value: T) -> Self
  where
    T: Send + Sync + 'static, {
    Self { inner: ArcShared::new(value), _marker: core::marker::PhantomData }
  }

  /// Returns the [`TypeId`] of the stored payload.
  #[must_use]
  pub fn type_id(&self) -> TypeId {
    (*self.inner).type_id()
  }

  /// Attempts to downcast the payload to the requested type.
  ///
  /// # Errors
  ///
  /// Returns `Err(Self)` when the stored value cannot be converted to `T`.
  pub fn try_downcast<T>(self) -> Result<ArcShared<T>, AdapterPayload<TB>>
  where
    T: Send + Sync + 'static, {
    match self.inner.downcast::<T>() {
      | Ok(concrete) => Ok(concrete),
      | Err(original) => Err(Self { inner: original, _marker: core::marker::PhantomData }),
    }
  }

  /// Returns a shared reference to the payload if it matches the requested type.
  #[must_use]
  pub fn downcast_ref<T>(&self) -> Option<&T>
  where
    T: 'static, {
    self.inner.downcast_ref::<T>()
  }

  /// Returns the erased payload.
  #[must_use]
  pub fn into_erased(self) -> ArcShared<dyn core::any::Any + Send + Sync + 'static> {
    self.inner
  }

  /// Creates a payload from an erased shared pointer.
  pub(crate) fn from_erased(inner: ArcShared<dyn core::any::Any + Send + Sync + 'static>) -> Self {
    Self { inner, _marker: core::marker::PhantomData }
  }
}

impl<TB: RuntimeToolbox> Clone for AdapterPayload<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: self._marker }
  }
}
