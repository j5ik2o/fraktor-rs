use alloc::string::String;

use crate::core::typed::delivery::ConsumerControllerDelivery;

#[test]
fn delivery_is_clone() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<ConsumerControllerDelivery<String>>();
}
