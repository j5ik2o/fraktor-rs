#[cfg(feature = "alloc")]
use super::VecRingStorage;
#[cfg(feature = "alloc")]
use crate::collections::queue::storage::QueueStorage;

#[cfg(feature = "alloc")]
#[test]
fn vec_ring_storage_try_grow_success() {
  let mut storage: VecRingStorage<u32> = VecRingStorage::with_capacity(5);
  assert_eq!(storage.capacity(), 5);

  let result = storage.try_grow(10);
  assert!(result.is_ok());
  assert_eq!(storage.capacity(), 10);
}

#[cfg(feature = "alloc")]
#[test]
fn vec_ring_storage_try_grow_no_change_when_smaller() {
  let mut storage: VecRingStorage<u32> = VecRingStorage::with_capacity(10);
  assert_eq!(storage.capacity(), 10);

  let result = storage.try_grow(5);
  assert!(result.is_ok());
  // ????????
  assert_eq!(storage.capacity(), 10);
}

#[cfg(feature = "alloc")]
#[test]
fn vec_ring_storage_try_grow_no_change_when_equal() {
  let mut storage: VecRingStorage<u32> = VecRingStorage::with_capacity(10);
  assert_eq!(storage.capacity(), 10);

  let result = storage.try_grow(10);
  assert!(result.is_ok());
  // ????????
  assert_eq!(storage.capacity(), 10);
}

#[cfg(feature = "alloc")]
#[test]
fn vec_ring_storage_read_unchecked() {
  let mut storage: VecRingStorage<u32> = VecRingStorage::with_capacity(5);
  storage.push_back(10);
  storage.push_back(20);
  storage.push_back(30);

  unsafe {
    let ptr0 = storage.read_unchecked(0);
    assert_eq!(*ptr0, 10);

    let ptr1 = storage.read_unchecked(1);
    assert_eq!( *ptr1, 20);

    let ptr2 = storage.read_unchecked(2);
    assert_eq!( *ptr2, 30);
  }
}

#[cfg(feature = "alloc")]
#[test]
fn vec_ring_storage_write_unchecked() {
  let mut storage: VecRingStorage<u32> = VecRingStorage::with_capacity(5);
  storage.push_back(10);
  storage.push_back(20);

  unsafe {
    storage.write_unchecked(0, 100);
    storage.write_unchecked(1, 200);
  }

  assert_eq!(storage.pop_front(), Some(100));
  assert_eq!(storage.pop_front(), Some(200));
}

#[cfg(feature = "alloc")]
#[test]
fn vec_ring_storage_write_unchecked_append() {
  let mut storage: VecRingStorage<u32> = VecRingStorage::with_capacity(5);
  storage.push_back(10);
  storage.push_back(20);

  unsafe {
    // idx == len() ????push_back
    storage.write_unchecked(2, 30);
  }

  assert_eq!(storage.len(), 3);
  assert_eq!(storage.pop_front(), Some(10));
  assert_eq!(storage.pop_front(), Some(20));
  assert_eq!(storage.pop_front(), Some(30));
}

#[cfg(feature = "alloc")]
#[test]
fn vec_ring_storage_read_unchecked_with_wraparound() {
  let mut storage: VecRingStorage<u32> = VecRingStorage::with_capacity(3);
  storage.push_back(10);
  storage.push_back(20);
  storage.push_back(30);

  // VecDeque?wraparound??????????
  let _ = storage.pop_front(); // 10???
  storage.push_back(40); // wraparound???????????

  unsafe {
    let ptr0 = storage.read_unchecked(0);
    let val0 = *ptr0;
    assert!(val0 == 20 || val0 == 30 || val0 == 40);
  }
}
