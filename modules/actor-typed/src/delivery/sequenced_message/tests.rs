use alloc::string::String;

use crate::delivery::SequencedMessage;

#[test]
fn sequenced_message_is_clone() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<SequencedMessage<String>>();
}
