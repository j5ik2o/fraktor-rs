use alloc::string::String;

use crate::delivery::ProducerControllerRequestNext;

// ProducerControllerRequestNext は実行中のアクターシステムが必要なため、
// delivery モジュールの統合テストでテストされる。

#[test]
fn request_next_accessor_consistency() {
  // コンパイル時チェック: 型がジェネリックかつ Clone であることを確認する。
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<ProducerControllerRequestNext<String>>();
}
