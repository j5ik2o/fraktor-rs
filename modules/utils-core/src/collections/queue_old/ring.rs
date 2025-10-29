mod arc_shared_ring_queue;
mod ring_backend;
mod ring_buffer;
mod ring_buffer_storage;
mod ring_handle;
mod ring_queue;
mod ring_storage_backend;

#[cfg(feature = "alloc")]
use core::{cell::RefCell, ops::Deref};

#[allow(unused_imports)]
pub use arc_shared_ring_queue::ArcSharedRingQueue;
pub use ring_backend::RingBackend;
pub use ring_buffer::{DEFAULT_CAPACITY, RingBuffer};
pub use ring_buffer_storage::RingBufferStorage;
pub use ring_handle::RingHandle;
pub use ring_queue::RingQueue;
pub use ring_storage_backend::RingStorageBackend;

#[cfg(feature = "alloc")]
use crate::{collections::queue_old::traits::QueueHandle, sync::RcShared};

#[cfg(feature = "alloc")]
impl<E> QueueHandle<E> for RcShared<RefCell<RingBuffer<E>>> {
  type Storage = RefCell<RingBuffer<E>>;

  fn storage(&self) -> &Self::Storage {
    self.deref()
  }
}

#[cfg(feature = "alloc")]
impl<E> RingHandle<E> for RcShared<RingStorageBackend<RcShared<RefCell<RingBuffer<E>>>>> {
  type Backend = RingStorageBackend<RcShared<RefCell<RingBuffer<E>>>>;

  fn backend(&self) -> &Self::Backend {
    self.deref()
  }
}
