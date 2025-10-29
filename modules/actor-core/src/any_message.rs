//! Untyped message container abstraction.

use core::any::{Any, TypeId};

/// Borrowed representation of an untyped message payload.
#[derive(Debug, Clone, Copy)]
pub struct AnyMessage<'a> {
  payload:  &'a dyn Any,
  metadata: Option<&'a dyn Any>,
  type_id:  TypeId,
}

impl<'a> AnyMessage<'a> {
  /// Creates a message view from a typed payload.
  #[must_use]
  pub fn from_ref<T: Any + 'a>(value: &'a T) -> Self {
    Self { payload: value, metadata: None, type_id: TypeId::of::<T>() }
  }

  /// Creates a message view from a dynamic payload and explicit metadata.
  #[must_use]
  pub fn from_dyn(payload: &'a dyn Any, metadata: Option<&'a dyn Any>, type_id: TypeId) -> Self {
    Self { payload, metadata, type_id }
  }

  /// Creates a message view with attached metadata.
  #[must_use]
  pub fn from_ref_with_metadata<T, M>(value: &'a T, metadata: &'a M) -> Self
  where
    T: Any + 'a,
    M: Any + 'a, {
    Self { payload: value, metadata: Some(metadata), type_id: TypeId::of::<T>() }
  }

  /// Returns the type identifier of the payload.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Attempts to borrow the payload as the requested type.
  #[must_use]
  pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
    if self.type_id == TypeId::of::<T>() { self.payload.downcast_ref::<T>() } else { None }
  }

  /// Returns the attached metadata if present.
  #[must_use]
  pub fn metadata(&self) -> Option<&'a dyn Any> {
    self.metadata
  }
}
