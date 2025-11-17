use super::*;

#[test]
fn rc_shared_new_and_deref() {
  let shared = RcShared::new(42);
  assert_eq!(*shared, 42);
}

#[test]
fn rc_shared_clone() {
  let shared1 = RcShared::new(42);
  let shared2 = shared1.clone();
  assert_eq!(*shared1, 42);
  assert_eq!(*shared2, 42);
}

#[test]
fn rc_shared_try_unwrap_success() {
  let shared = RcShared::new(42);
  match shared.try_unwrap() {
    | Ok(value) => assert_eq!(value, 42),
    | Err(_) => panic!("try_unwrap should succeed"),
  }
}

#[test]
fn rc_shared_try_unwrap_failure() {
  let shared1 = RcShared::new(42);
  let shared2 = shared1.clone();
  let result = shared1.try_unwrap();
  assert!(result.is_err());
  assert_eq!(*shared2, 42);
}

#[test]
fn rc_shared_from_rc_and_into_rc() {
  use alloc::rc::Rc;
  let rc = Rc::new(42);
  let shared = RcShared::___from_rc(rc);
  assert_eq!(*shared, 42);
  let rc_back = shared.___into_rc();
  assert_eq!(*rc_back, 42);
}

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
  fn rc_shared_into_dyn() {
    let shared = RcShared::new(42i32);
    let dyn_shared: RcShared<dyn TestTrait> = shared.into_dyn(|v| v as &dyn TestTrait);
    assert_eq!(dyn_shared.get_value(), 42);
  }

  #[test]
  #[allow(deprecated)]
  fn rc_shared_shared_dyn_into_dyn() {
    let shared = RcShared::new(42i32);
    let dyn_shared: RcShared<dyn TestTrait> = SharedDyn::into_dyn(shared, |v| v as &dyn TestTrait);
    assert_eq!(dyn_shared.get_value(), 42);
  }
}
