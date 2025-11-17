use alloc::format;

use super::SerializationError;
use crate::core::serialization::{
  call_scope::SerializationCallScope, not_serializable_error::NotSerializableError, serializer_id::SerializerId,
};

#[test]
fn invalid_format_debug_representation() {
  let error = SerializationError::InvalidFormat;
  let debug = format!("{error:?}");
  assert!(debug.contains("InvalidFormat"));
}

#[test]
fn not_serializable_variant_embeds_payload() {
  let payload = NotSerializableError::new(
    "Example",
    Some(SerializerId::try_from(41).unwrap()),
    Some("manifest".into()),
    None,
    None,
  );
  let error = SerializationError::NotSerializable(payload.clone());
  match error {
    | SerializationError::NotSerializable(inner) => {
      assert_eq!(inner.type_name(), "Example");
      assert_eq!(inner.manifest(), Some("manifest"));
      assert_eq!(inner.serializer_id(), Some(SerializerId::try_from(41).unwrap()));
    },
    | _ => panic!("unexpected variant"),
  }
}

#[test]
fn const_constructors_create_correct_variants() {
  // Uninitialized コンストラクタのテスト
  let error = SerializationError::uninitialized();
  assert!(error.is_uninitialized());

  // ManifestMissing コンストラクタのテスト
  let error = SerializationError::manifest_missing(SerializationCallScope::Local);
  assert!(error.is_manifest_missing());

  // UnknownSerializer コンストラクタのテスト
  let error = SerializationError::unknown_serializer(SerializerId::try_from(42).unwrap());
  assert!(error.is_unknown_serializer());

  // InvalidFormat コンストラクタのテスト
  let error = SerializationError::invalid_format();
  assert!(error.is_invalid_format());

  // UnknownManifest コンストラクタのテスト
  let error = SerializationError::unknown_manifest("test_manifest");
  assert!(error.is_unknown_manifest());

  // NotSerializable コンストラクタのテスト
  let payload = NotSerializableError::new("Example", None, None, None, None);
  let error = SerializationError::not_serializable(payload);
  assert!(error.is_not_serializable());
}

#[test]
fn is_methods_return_correct_values() {
  // すべてのバリアントで is_* メソッドが正しく動作することを確認
  let uninitialized = SerializationError::Uninitialized;
  assert!(uninitialized.is_uninitialized());
  assert!(!uninitialized.is_manifest_missing());
  assert!(!uninitialized.is_unknown_serializer());
  assert!(!uninitialized.is_not_serializable());
  assert!(!uninitialized.is_unknown_manifest());
  assert!(!uninitialized.is_invalid_format());

  let manifest_missing = SerializationError::ManifestMissing { scope: SerializationCallScope::Local };
  assert!(!manifest_missing.is_uninitialized());
  assert!(manifest_missing.is_manifest_missing());
  assert!(!manifest_missing.is_unknown_serializer());

  let unknown_serializer = SerializationError::UnknownSerializer(SerializerId::try_from(100).unwrap());
  assert!(!unknown_serializer.is_uninitialized());
  assert!(!unknown_serializer.is_manifest_missing());
  assert!(unknown_serializer.is_unknown_serializer());

  let invalid_format = SerializationError::InvalidFormat;
  assert!(!invalid_format.is_uninitialized());
  assert!(invalid_format.is_invalid_format());
}
