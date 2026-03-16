use alloc::string::String;

use crate::core::typed::delivery::ProducerControllerRequestNext;

// ProducerControllerRequestNext is tested through integration tests
// in the delivery module tests since it requires a running actor system.

#[test]
fn request_next_accessor_consistency() {
  // Compile-time check: ensure the type is generic and Clone.
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<ProducerControllerRequestNext<String>>();
}
