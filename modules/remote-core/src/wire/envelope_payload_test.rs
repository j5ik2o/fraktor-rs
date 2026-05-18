use alloc::string::String;

use bytes::Bytes;

use super::EnvelopePayload;

#[test]
fn accessors_return_payload_metadata() {
  let bytes = Bytes::from_static(b"payload");
  let payload = EnvelopePayload::new(42, Some(String::from("example.Manifest")), bytes.clone());

  assert_eq!(payload.serializer_id(), 42);
  assert_eq!(payload.manifest(), Some("example.Manifest"));
  assert_eq!(payload.bytes(), &bytes);
}

#[test]
fn accessors_preserve_absent_manifest() {
  let bytes = Bytes::from_static(b"payload");
  let payload = EnvelopePayload::new(7, None, bytes.clone());

  assert_eq!(payload.serializer_id(), 7);
  assert_eq!(payload.manifest(), None);
  assert_eq!(payload.bytes(), &bytes);
}
