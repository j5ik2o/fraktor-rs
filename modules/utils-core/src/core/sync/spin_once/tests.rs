use super::SpinOnce;
use crate::core::sync::OnceDriver;

#[test]
fn test_spin_once_new_is_uncompleted() {
  let once: SpinOnce<i32> = SpinOnce::new();
  assert!(!once.is_completed());
  assert!(once.get().is_none());
}

#[test]
fn test_spin_once_call_once_initializes_value() {
  let once = SpinOnce::new();
  let value = once.call_once(|| 42);
  assert_eq!(*value, 42);
  assert!(once.is_completed());
  assert_eq!(once.get(), Some(&42));
}

#[test]
fn test_spin_once_call_once_runs_only_once() {
  let once = SpinOnce::new();
  let first = *once.call_once(|| 10);
  let second = *once.call_once(|| 99);
  assert_eq!(first, 10);
  assert_eq!(second, 10);
  assert_eq!(once.get(), Some(&10));
}

#[test]
fn test_spin_once_get_before_init_is_none() {
  let once: SpinOnce<&str> = SpinOnce::new();
  assert!(once.get().is_none());
  let _ = once.call_once(|| "value");
  assert_eq!(once.get(), Some(&"value"));
}

#[test]
fn test_spin_once_const_new() {
  static ONCE: SpinOnce<i32> = SpinOnce::new();
  let value = ONCE.call_once(|| 7);
  assert_eq!(*value, 7);
  assert!(ONCE.is_completed());
}

#[test]
fn test_spin_once_driver_trait() {
  let once: SpinOnce<u8> = <SpinOnce<u8> as OnceDriver<u8>>::new();
  assert!(!OnceDriver::is_completed(&once));
  assert_eq!(OnceDriver::get(&once), None);
  let value = OnceDriver::call_once(&once, || 5u8);
  assert_eq!(*value, 5);
  assert!(OnceDriver::is_completed(&once));
  assert_eq!(OnceDriver::get(&once), Some(&5));
}
