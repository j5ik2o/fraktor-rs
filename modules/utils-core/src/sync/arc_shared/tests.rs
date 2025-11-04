use super::*;

#[test]
fn arc_shared_new_and_deref() {
  let shared = ArcShared::new(42);
  assert_eq!(*shared, 42);
}

#[test]
fn arc_shared_clone() {
  let shared1 = ArcShared::new(42);
  let shared2 = shared1.clone();
  assert_eq!(*shared1, 42);
  assert_eq!(*shared2, 42);
}

#[test]
fn arc_shared_try_unwrap_success() {
  let shared = ArcShared::new(42);
  match shared.try_unwrap() {
    | Ok(value) => assert_eq!(value, 42),
    | Err(_) => panic!("try_unwrap should succeed"),
  }
}

#[test]
fn arc_shared_try_unwrap_failure() {
  let shared1 = ArcShared::new(42);
  let shared2 = shared1.clone();
  let result = shared1.try_unwrap();
  assert!(result.is_err());
  assert_eq!(*shared2, 42);
}

#[test]
fn arc_shared_from_arc_and_into_arc() {
  #[cfg(not(feature = "force-portable-arc"))]
  use alloc::sync::Arc;

  #[cfg(feature = "force-portable-arc")]
  use portable_atomic_util::Arc;

  let arc: Arc<i32> = Arc::new(42);
  let shared: ArcShared<i32> = ArcShared::___from_arc(arc);
  assert_eq!(*shared, 42);
  let arc_back = shared.___into_arc();
  assert_eq!(*arc_back, 42);
}

#[test]
fn arc_shared_into_raw_and_from_raw() {
  let shared = ArcShared::new(42);
  let raw = shared.into_raw();
  let restored = unsafe { ArcShared::from_raw(raw) };
  assert_eq!(*restored, 42);
}

#[cfg(not(feature = "unsize"))]
mod trait_object_tests {
  use super::*;

  trait TestTrait {
    fn get_value(&self) -> i32;
  }

  impl TestTrait for i32 {
    fn get_value(&self) -> i32 {
      *self
    }
  }

  #[test]
  fn arc_shared_into_dyn() {
    let shared = ArcShared::new(42i32);
    let dyn_shared: ArcShared<dyn TestTrait> = shared.into_dyn(|v| v as &dyn TestTrait);
    assert_eq!(dyn_shared.get_value(), 42);
  }

  #[test]
  fn arc_shared_shared_dyn_into_dyn() {
    let shared = ArcShared::new(42i32);
    let dyn_shared: ArcShared<dyn TestTrait> = SharedDyn::into_dyn(shared, |v| v as &dyn TestTrait);
    assert_eq!(dyn_shared.get_value(), 42);
  }
}
