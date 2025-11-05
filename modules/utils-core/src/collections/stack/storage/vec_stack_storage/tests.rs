use super::*;

#[test]
fn with_capacity_creates_empty_storage() {
  let storage: VecStackStorage<i32> = VecStackStorage::with_capacity(10);
  assert_eq!(storage.len(), 0);
  assert_eq!(storage.capacity(), 10);
  assert!(storage.is_empty());
}

#[test]
fn push_and_pop_operations() {
  let mut storage = VecStackStorage::with_capacity(5);

  storage.push(1);
  storage.push(2);
  storage.push(3);

  assert_eq!(storage.len(), 3);
  assert!(!storage.is_empty());

  assert_eq!(storage.pop(), Some(3));
  assert_eq!(storage.pop(), Some(2));
  assert_eq!(storage.pop(), Some(1));
  assert_eq!(storage.pop(), None);
  assert!(storage.is_empty());
}

#[test]
fn peek_returns_last_element() {
  let mut storage = VecStackStorage::with_capacity(5);

  assert_eq!(storage.peek(), None);

  storage.push(10);
  assert_eq!(storage.peek(), Some(&10));

  storage.push(20);
  assert_eq!(storage.peek(), Some(&20));

  storage.pop();
  assert_eq!(storage.peek(), Some(&10));
}

#[test]
fn try_grow_increases_capacity() {
  let mut storage = VecStackStorage::with_capacity(5);

  storage.push(1);
  storage.push(2);

  assert_eq!(storage.capacity(), 5);

  assert!(storage.try_grow(10).is_ok());
  assert_eq!(storage.capacity(), 10);

  // Growing to same or smaller capacity is no-op
  assert!(storage.try_grow(8).is_ok());
  assert_eq!(storage.capacity(), 10);

  // Data should be preserved
  assert_eq!(storage.len(), 2);
  assert_eq!(storage.pop(), Some(2));
  assert_eq!(storage.pop(), Some(1));
}

#[test]
fn stack_storage_trait_capacity() {
  let storage: VecStackStorage<i32> = VecStackStorage::with_capacity(15);
  assert_eq!(StackStorage::capacity(&storage), 15);
}

#[test]
fn stack_storage_trait_read_unchecked() {
  let mut storage = VecStackStorage::with_capacity(5);
  storage.push(100);
  storage.push(200);

  unsafe {
    let ptr = StackStorage::read_unchecked(&storage, 0);
    assert_eq!(*ptr, 100);

    let ptr = StackStorage::read_unchecked(&storage, 1);
    assert_eq!(*ptr, 200);
  }
}

#[test]
fn stack_storage_trait_write_unchecked_at_end() {
  let mut storage = VecStackStorage::with_capacity(5);
  storage.push(1);

  unsafe {
    // Write at index == len() should push
    StackStorage::write_unchecked(&mut storage, 1, 2);
  }

  assert_eq!(storage.len(), 2);
  assert_eq!(storage.pop(), Some(2));
  assert_eq!(storage.pop(), Some(1));
}

#[test]
fn stack_storage_trait_write_unchecked_in_middle() {
  let mut storage = VecStackStorage::with_capacity(5);
  storage.push(10);
  storage.push(20);
  storage.push(30);

  unsafe {
    // Overwrite middle element
    StackStorage::write_unchecked(&mut storage, 1, 99);
  }

  assert_eq!(storage.len(), 3);
  assert_eq!(storage.pop(), Some(30));
  assert_eq!(storage.pop(), Some(99));
  assert_eq!(storage.pop(), Some(10));
}
