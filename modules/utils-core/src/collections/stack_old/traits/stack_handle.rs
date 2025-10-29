use super::stack_backend::StackBackend;
use crate::sync::Shared;

/// Handle that exposes a [`StackBackend`].
pub trait StackHandle<T>: Shared<Self::Backend> + Clone {
  /// Backend type managed by this handle.
  type Backend: StackBackend<T> + ?Sized;

  /// Gets a reference to the backend.
  fn backend(&self) -> &Self::Backend;
}
