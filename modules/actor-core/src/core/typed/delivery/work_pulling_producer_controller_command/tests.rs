use alloc::string::String;

use crate::core::typed::delivery::WorkPullingProducerControllerCommand;

#[test]
fn command_clone_compiles() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<WorkPullingProducerControllerCommand<String>>();
}
