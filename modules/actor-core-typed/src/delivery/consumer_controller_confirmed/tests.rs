use super::ConsumerControllerConfirmed;

#[test]
fn confirmed_is_copy() {
  let a = ConsumerControllerConfirmed;
  let b = a;
  assert_eq!(a, b);
}
