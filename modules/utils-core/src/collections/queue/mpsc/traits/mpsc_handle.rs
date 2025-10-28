use crate::{collections::queue::mpsc::traits::mpsc_backend::MpscBackend, sync::Shared};

/// Shared handle trait exposing an [`MpscBackend`].
pub trait MpscHandle<T>: Shared<Self::Backend> + Clone {
  /// Backend type referenced by this handle.
  type Backend: MpscBackend<T> + ?Sized;

  /// Gets a reference to the backend managed by this handle.
  fn backend(&self) -> &Self::Backend;
}
