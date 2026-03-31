//! Human readable explanation associated with an actor error.

#[cfg(test)]
mod tests;

use alloc::{borrow::Cow, string::String};
use core::{any::TypeId, fmt};

/// Describes the reason behind an actor failure.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActorErrorReason {
  message:        Cow<'static, str>,
  source_type_id: Option<TypeId>,
}

impl ActorErrorReason {
  /// Creates a new error reason from the provided message.
  #[must_use]
  pub fn new(reason: impl Into<Cow<'static, str>>) -> Self {
    Self { message: reason.into(), source_type_id: None }
  }

  /// Creates a new error reason tagged with the source error's type identity.
  #[must_use]
  pub fn typed<E: 'static>(reason: impl Into<Cow<'static, str>>) -> Self {
    Self { message: reason.into(), source_type_id: Some(TypeId::of::<E>()) }
  }

  /// Returns the underlying message as a string slice.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // Cow<str> の Deref が const でないため const fn にできない
  pub fn as_str(&self) -> &str {
    &self.message
  }

  /// Returns the type identity of the original error, if recorded.
  #[must_use]
  pub const fn source_type_id(&self) -> Option<TypeId> {
    self.source_type_id
  }

  /// Returns `true` when the source error matches the provided type.
  #[must_use]
  pub fn is_source_type<E: 'static>(&self) -> bool {
    self.source_type_id == Some(TypeId::of::<E>())
  }
}

impl From<&'static str> for ActorErrorReason {
  fn from(value: &'static str) -> Self {
    Self::new(Cow::Borrowed(value))
  }
}

impl From<String> for ActorErrorReason {
  fn from(value: String) -> Self {
    Self::new(Cow::Owned(value))
  }
}

impl From<Cow<'static, str>> for ActorErrorReason {
  fn from(value: Cow<'static, str>) -> Self {
    Self::new(value)
  }
}

impl fmt::Display for ActorErrorReason {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.message)
  }
}
