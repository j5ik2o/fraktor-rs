//! Adapter payload wrapper for erased message values.

#[cfg(test)]
mod tests;

use core::{
  any::{Any, TypeId},
  marker::PhantomData,
};

use fraktor_utils_core_rs::sync::ArcShared;

/// Owns a dynamically typed payload destined for message adapters.
#[derive(Debug)]
pub struct AdapterPayload {
  inner:   ArcShared<dyn Any + Send + Sync + 'static>,
  _marker: PhantomData<()>,
}

impl AdapterPayload {
  /// Creates a payload from the provided message.
  #[must_use]
  pub fn new<T>(value: T) -> Self
  where
    T: Send + Sync + 'static, {
    Self { inner: ArcShared::new(value), _marker: PhantomData }
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
  pub fn try_downcast<T>(self) -> Result<ArcShared<T>, AdapterPayload>
  where
    T: Send + Sync + 'static, {
    match self.inner.downcast::<T>() {
      | Ok(concrete) => Ok(concrete),
      | Err(original) => Err(Self { inner: original, _marker: PhantomData }),
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
  pub fn into_erased(self) -> ArcShared<dyn Any + Send + Sync + 'static> {
    self.inner
  }

  /// Creates a payload from an erased shared pointer.
  pub(crate) fn from_erased(inner: ArcShared<dyn Any + Send + Sync + 'static>) -> Self {
    Self { inner, _marker: PhantomData }
  }
}

impl Clone for AdapterPayload {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: self._marker }
  }
}
