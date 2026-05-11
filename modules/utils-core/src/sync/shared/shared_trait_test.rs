use super::Shared;
use crate::sync::arc_shared::ArcShared;

#[test]
fn shared_with_ref_reads_value() {
  let shared = ArcShared::new(42);
  let value = shared.with_ref(|v| *v);
  assert_eq!(value, 42);
}

#[test]
fn shared_with_ref_modifies_through_closure() {
  let shared = ArcShared::new(10);
  let doubled = shared.with_ref(|v| *v * 2);
  assert_eq!(doubled, 20);
  assert_eq!(*shared, 10);
}

#[test]
fn shared_try_unwrap_single_reference() {
  let shared = ArcShared::new(100);
  match shared.try_unwrap() {
    | Ok(value) => assert_eq!(value, 100),
    | Err(_) => panic!("try_unwrap should succeed with single reference"),
  }
}

#[test]
fn shared_try_unwrap_multiple_references() {
  let shared1 = ArcShared::new(200);
  let shared2 = shared1.clone();
  match shared1.try_unwrap() {
    | Ok(_) => panic!("try_unwrap should fail with multiple references"),
    | Err(remaining) => {
      assert_eq!(*remaining, 200);
      assert_eq!(*shared2, 200);
    },
  }
}

#[test]
fn shared_deref_provides_access() {
  let shared = ArcShared::new(300);
  assert_eq!(*shared, 300);
}

#[test]
fn shared_clone_creates_shared_reference() {
  let shared1 = ArcShared::new(400);
  let shared2 = shared1.clone();
  assert_eq!(*shared1, 400);
  assert_eq!(*shared2, 400);
}
