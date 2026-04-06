use alloc::string::String;

use crate::core::typed::delivery::SequencedMessage;

#[test]
fn sequenced_message_is_clone() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<SequencedMessage<String>>();
}
