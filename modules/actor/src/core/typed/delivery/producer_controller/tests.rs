use alloc::string::String;

use crate::core::typed::delivery::{ProducerController, ProducerControllerCommand};

#[test]
fn producer_controller_factory_methods_compile() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<ProducerControllerCommand<String>>();

  // Ensure the behavior factory compiles with a concrete type.
  let _behavior = ProducerController::behavior::<u32>("test-producer");
}
