use alloc::string::String;

use crate::delivery::{ConfirmationQualifier, NO_QUALIFIER};

#[test]
fn no_qualifier_is_empty_string() {
  // 前提・実行: NO_QUALIFIER 定数を参照する
  let qualifier: &ConfirmationQualifier = &NO_QUALIFIER;

  // 確認: 空文字列である
  assert!(qualifier.is_empty());
}

#[test]
fn confirmation_qualifier_is_string_alias() {
  // 前提: 文字列から ConfirmationQualifier を構築する
  let qualifier: ConfirmationQualifier = String::from("topic-A");

  // 確認: String として振る舞う
  assert_eq!(qualifier.as_str(), "topic-A");
}

#[test]
fn no_qualifier_equals_empty_string() {
  // 前提: NO_QUALIFIER と空文字列を用意する
  let empty: ConfirmationQualifier = String::from("");

  // 確認: 両者は等しい
  assert_eq!(NO_QUALIFIER, empty);
}
