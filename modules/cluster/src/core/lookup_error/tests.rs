use alloc::string::String;

use crate::core::lookup_error::LookupError;

#[test]
fn no_authority_variant_exists() {
  // NoAuthority バリアントが正しく作成できることを確認
  let err = LookupError::NoAuthority;
  assert_eq!(err, LookupError::NoAuthority);
}

#[test]
fn activation_failed_holds_key() {
  // ActivationFailed バリアントが GrainKey 情報を保持することを確認
  let err = LookupError::ActivationFailed { key: String::from("user:123") };
  match err {
    | LookupError::ActivationFailed { key } => {
      assert_eq!(key, "user:123");
    },
    | _ => panic!("Expected ActivationFailed variant"),
  }
}

#[test]
fn timeout_variant_exists() {
  // Timeout バリアントが正しく作成できることを確認
  let err = LookupError::Timeout;
  assert_eq!(err, LookupError::Timeout);
}

#[test]
fn debug_is_implemented() {
  // Debug トレイトが実装されていることを確認
  let err = LookupError::ActivationFailed { key: String::from("test:key") };
  let debug_str = alloc::format!("{:?}", err);
  assert!(debug_str.contains("ActivationFailed"));
  assert!(debug_str.contains("test:key"));
}

#[test]
fn clone_is_implemented() {
  // Clone トレイトが実装されていることを確認
  let err = LookupError::ActivationFailed { key: String::from("clone:test") };
  let cloned = err.clone();
  assert_eq!(err, cloned);
}

#[test]
fn partial_eq_works_correctly() {
  // PartialEq トレイトが正しく動作することを確認
  let err1 = LookupError::ActivationFailed { key: String::from("same:key") };
  let err2 = LookupError::ActivationFailed { key: String::from("same:key") };
  let err3 = LookupError::ActivationFailed { key: String::from("different:key") };

  assert_eq!(err1, err2);
  assert_ne!(err1, err3);
  assert_ne!(LookupError::NoAuthority, LookupError::Timeout);
}
