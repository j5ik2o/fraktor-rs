use crate::{Shared, collections::queue::traits::QueueStorage};

/// Common interface for queue handles.
pub trait QueueHandle<E>: Shared<Self::Storage> + Clone {
  /// Storage backend type used by the handle.
  type Storage: QueueStorage<E> + ?Sized;

  /// Gets a reference to the internal storage.
  fn storage(&self) -> &Self::Storage;
}
