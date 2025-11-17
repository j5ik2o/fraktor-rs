use super::GuardHandle;

#[test]
fn new_creates_guard_handle() {
  let handle = GuardHandle::new(42);
  assert_eq!(*handle, 42);
}

#[test]
fn into_inner_extracts_value() {
  let handle = GuardHandle::new(100);
  let value = handle.into_inner();
  assert_eq!(value, 100);
}

#[test]
fn deref_allows_read() {
  let handle = GuardHandle::new(String::from("hello"));
  assert_eq!(handle.len(), 5);
  assert_eq!(handle.as_str(), "hello");
}

#[test]
fn deref_mut_allows_write() {
  let mut handle = GuardHandle::new(String::from("hello"));
  handle.push_str(" world");
  assert_eq!(handle.as_str(), "hello world");
}

#[test]
fn debug_format() {
  let handle = GuardHandle::new(42);
  let debug_str = format!("{:?}", handle);
  assert!(debug_str.contains("GuardHandle"));
}
