use crate::core::kernel::actor::messaging::ReceiveTimeout;

#[test]
fn receive_timeout_is_cloneable_and_equatable() {
  let left = ReceiveTimeout;
  let right = left;

  assert_eq!(left, right);
}
