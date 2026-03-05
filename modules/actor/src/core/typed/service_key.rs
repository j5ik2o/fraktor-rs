//! Type-safe service key for actor discovery.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::{any::TypeId, marker::PhantomData};

/// Type-safe key for registering and discovering actors in the Receptionist.
///
/// Each `ServiceKey` carries a string identifier and the compile-time message type `M`.
/// Two keys match when both their id and message type are equal.
#[derive(Debug)]
pub struct ServiceKey<M>
where
  M: Send + Sync + 'static, {
  id:      String,
  type_id: TypeId,
  marker:  PhantomData<fn() -> M>,
}

impl<M> ServiceKey<M>
where
  M: Send + Sync + 'static,
{
  /// Creates a new service key with the given identifier.
  #[must_use]
  pub fn new(id: impl Into<String>) -> Self {
    Self { id: id.into(), type_id: TypeId::of::<M>(), marker: PhantomData }
  }

  /// Returns the service identifier.
  #[must_use]
  pub fn id(&self) -> &str {
    &self.id
  }

  /// Returns the [`TypeId`] of the message type `M`.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }
}

impl<M> Clone for ServiceKey<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self { id: self.id.clone(), type_id: self.type_id, marker: PhantomData }
  }
}

impl<M> PartialEq for ServiceKey<M>
where
  M: Send + Sync + 'static,
{
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.type_id == other.type_id
  }
}

impl<M> Eq for ServiceKey<M> where M: Send + Sync + 'static {}
