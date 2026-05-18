use alloc::string::String;

use crate::delivery::ConsumerControllerCommand;

#[test]
fn command_is_clone() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<ConsumerControllerCommand<String>>();
}
