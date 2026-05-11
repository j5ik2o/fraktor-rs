use alloc::string::String;

use crate::delivery::ProducerControllerCommand;

#[test]
fn command_is_clone() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<ProducerControllerCommand<String>>();
}
