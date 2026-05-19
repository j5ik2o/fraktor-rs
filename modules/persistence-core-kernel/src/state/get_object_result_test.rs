use crate::state::GetObjectResult;

#[test]
fn get_object_result_exposes_value_and_revision() {
  let result = GetObjectResult::new(Some(42), 7);

  assert_eq!(result.value(), Some(&42));
  assert_eq!(result.revision(), 7);
  assert!(!result.is_empty());
  assert_eq!(result.into_value(), Some(42));
}

#[test]
fn empty_get_object_result_has_no_value_and_zero_revision() {
  let result = GetObjectResult::<i32>::empty();

  assert_eq!(result.value(), None);
  assert_eq!(result.revision(), 0);
  assert!(result.is_empty());
}
