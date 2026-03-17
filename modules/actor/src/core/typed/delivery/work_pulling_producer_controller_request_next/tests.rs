use alloc::string::String;

use crate::core::typed::delivery::WorkPullingProducerControllerRequestNext;

#[test]
fn request_next_clone_compiles() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<WorkPullingProducerControllerRequestNext<String>>();
}
