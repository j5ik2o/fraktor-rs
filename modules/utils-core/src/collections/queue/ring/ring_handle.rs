use crate::{RingBackend, Shared};

/// Handle trait that provides references to [`RingBackend`].
pub trait RingHandle<E>: Shared<Self::Backend> + Clone {
  /// Backend type referenced by this handle.
  type Backend: RingBackend<E> + ?Sized;

  /// Gets a reference to the backend.
  fn backend(&self) -> &Self::Backend;
}
