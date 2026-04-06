use super::{SerializerId, SerializerIdError};

#[test]
fn rejects_reserved_range() {
  let result = SerializerId::try_from(10);
  assert_eq!(result, Err(SerializerIdError::Reserved(10)));
}

#[test]
fn accepts_non_reserved_values() {
  let result = SerializerId::try_from(100);
  assert!(result.is_ok(), "expected >40 identifiers to be allowed");
}
