#[cfg(feature = "alloc")]
use super::SyncSpscProducerShared;
#[cfg(feature = "alloc")]
use crate::core::collections::queue::{
  OverflowPolicy, QueueError, SyncQueue, backend::VecDequeBackend, type_keys::SpscKey,
};
#[cfg(feature = "alloc")]
use crate::core::sync::{ArcShared, SharedAccess, sync_mutex_like::SpinSyncMutex};

#[cfg(feature = "alloc")]
#[test]
fn sync_spsc_producer_offer_success() {
  let backend = VecDequeBackend::with_capacity(10, OverflowPolicy::DropOldest);
  let sync_queue = SyncQueue::new(backend);
  let mutex = ArcShared::new(SpinSyncMutex::new(sync_queue));
  let producer = SyncSpscProducerShared::new(mutex.clone());

  let result = producer.offer(42);
  assert!(result.is_ok());

  let queue_len = mutex.with_read(|q: &SyncQueue<u32, SpscKey, VecDequeBackend<u32>>| q.len());
  assert_eq!(queue_len, 1);
}

#[cfg(feature = "alloc")]
#[test]
fn sync_spsc_producer_offer_closed() {
  let backend = VecDequeBackend::with_capacity(10, OverflowPolicy::DropOldest);
  let sync_queue = SyncQueue::new(backend);
  let mutex = ArcShared::new(SpinSyncMutex::new(sync_queue));
  let producer = SyncSpscProducerShared::new(mutex.clone());

  mutex.with_write(|q: &mut SyncQueue<u32, SpscKey, VecDequeBackend<u32>>| q.close()).unwrap();

  let result = producer.offer(42);
  assert!(result.is_err());
  match result.unwrap_err() {
    | QueueError::Closed(item) => assert_eq!(item, 42),
    | _ => panic!("Expected Closed error"),
  }
}
