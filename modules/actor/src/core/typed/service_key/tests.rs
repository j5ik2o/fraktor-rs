use crate::core::typed::service_key::ServiceKey;

#[test]
fn should_create_with_id() {
  let key = ServiceKey::<u32>::new("my-service");
  assert_eq!(key.id(), "my-service");
}

#[test]
fn should_equal_same_type_and_id() {
  let a = ServiceKey::<u32>::new("svc");
  let b = ServiceKey::<u32>::new("svc");
  assert_eq!(a, b);
}

#[test]
fn should_differ_by_id() {
  let a = ServiceKey::<u32>::new("a");
  let b = ServiceKey::<u32>::new("b");
  assert_ne!(a, b);
}

#[test]
fn should_clone() {
  let key = ServiceKey::<u32>::new("cloned");
  let cloned = key.clone();
  assert_eq!(key, cloned);
}

#[test]
fn should_have_correct_type_id() {
  let key = ServiceKey::<String>::new("typed");
  assert_eq!(key.type_id(), core::any::TypeId::of::<String>());
}
