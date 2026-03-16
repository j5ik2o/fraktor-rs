use alloc::string::String;

use crate::core::typed::delivery::{ConsumerController, ConsumerControllerCommand};

#[test]
fn consumer_controller_factory_methods_compile() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<ConsumerControllerCommand<String>>();

  // 具体的な型でビヘイビアファクトリがコンパイルできることを確認する。
  let _behavior = ConsumerController::behavior::<u32>();
}
