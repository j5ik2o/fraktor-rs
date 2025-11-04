#[cfg(feature = "alloc")]
use super::SyncSpscProducer;
#[cfg(feature = "alloc")]
use crate::{
  collections::queue::{
    OverflowPolicy, QueueError, VecRingStorage,
    backend::{SyncQueueBackend, VecRingBackend},
  },
  sync::{ArcShared, Shared, SharedAccess, sync_mutex_like::SpinSyncMutex},
};

#[cfg(feature = "alloc")]
#[test]
fn sync_spsc_producer_offer_success() {
  let storage = VecRingStorage::with_capacity(10);
  let backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::DropOldest);
  let mutex = ArcShared::new(SpinSyncMutex::new(backend));
  let producer = SyncSpscProducer::new(mutex.clone());

  let result = producer.offer(42);
  assert!(result.is_ok());

  let queue_len =
    <ArcShared<SpinSyncMutex<VecRingBackend<u32>>> as Shared<SpinSyncMutex<VecRingBackend<u32>>>>::with_ref(
      &mutex,
      |m| m.lock().len(),
    );
  assert_eq!(queue_len, 1);
}

#[cfg(feature = "alloc")]
#[test]
fn sync_spsc_producer_offer_closed() {
  let storage = VecRingStorage::with_capacity(10);
  let backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::DropOldest);
  let mutex = ArcShared::new(SpinSyncMutex::new(backend));
  let producer = SyncSpscProducer::new(mutex.clone());

  // ??????????
  mutex.with_mut(|b: &mut VecRingBackend<u32>| b.close()).unwrap();

  let result = producer.offer(42);
  assert!(result.is_err());
  match result.unwrap_err() {
    | QueueError::Closed(item) => assert_eq!(item, 42),
    | _ => panic!("Expected Closed error"),
  }
}
