use alloc::format;

use super::SerializationError;
use crate::serialization::{not_serializable_error::NotSerializableError, serializer_id::SerializerId};

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
