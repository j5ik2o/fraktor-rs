use alloc::string::String;

use crate::core::typed::delivery::ProducerControllerCommand;

#[test]
fn command_is_clone() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<ProducerControllerCommand<String>>();
}
