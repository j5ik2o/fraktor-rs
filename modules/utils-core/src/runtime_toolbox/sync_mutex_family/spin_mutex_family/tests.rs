use super::*;

#[test]
fn test_spin_mutex_family_default() {
  let _family = SpinMutexFamily::default();
}

#[test]
fn test_spin_mutex_family_clone() {
  let family = SpinMutexFamily;
  let _cloned = family;
}

#[test]
fn test_spin_mutex_family_debug() {
  let family = SpinMutexFamily;
  let debug_str = format!("{:?}", family);
  assert!(debug_str.contains("SpinMutexFamily"));
}

#[test]
fn test_spin_mutex_family_create() {
  let mutex = SpinMutexFamily::create(42i32);

  // Test that the created mutex works
  let guard = mutex.lock();
  assert_eq!(*guard, 42);
}

#[test]
fn test_spin_mutex_family_create_with_string() {
  let mutex = SpinMutexFamily::create("test".to_string());

  let guard = mutex.lock();
  assert_eq!(guard.as_str(), "test");
}

#[test]
fn test_spin_mutex_family_type_association() {
  // Test that the type association works correctly
  fn _uses_spin_mutex_family<M>()
  where
    M: SyncMutexFamily, {
    let _mutex = M::create(123u64);
  }

  _uses_spin_mutex_family::<SpinMutexFamily>();
}
