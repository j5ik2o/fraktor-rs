use serde::{Deserialize, Serialize};

use super::BincodeSerializer;
use crate::serialization::serializer::SerializerImpl;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Sample(u32);

#[test]
fn serialize_erased_works() {
  let serializer = BincodeSerializer::new();
  let sample = Sample(7);
  let erased: &dyn erased_serde::Serialize = &sample;
  let bytes = serializer.serialize_erased(erased).expect("serialize");
  let decoded: Sample =
    bincode::serde::decode_from_slice(bytes.as_ref(), bincode::config::standard()).expect("decode").0;
  assert_eq!(decoded, sample);
}

#[test]
fn deserialize_is_not_supported_without_manifest() {
  let serializer = BincodeSerializer::new();
  let bytes = [0_u8; 4];
  let error = serializer.deserialize(&bytes, "MyType").expect_err("should fail");
  match error {
    | crate::serialization::error::SerializationError::UnknownManifest { serializer_id, manifest } => {
      assert_eq!(serializer_id, 1);
      assert_eq!(manifest, "MyType");
    },
    | other => panic!("unexpected error: {other:?}"),
  }
}
