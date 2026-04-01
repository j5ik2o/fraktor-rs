use crate::core::typed::delivery::{ConfirmationQualifier, NO_QUALIFIER};

#[test]
fn no_qualifier_is_empty_string() {
  // Given/When: the NO_QUALIFIER constant
  let qualifier: &ConfirmationQualifier = &NO_QUALIFIER;

  // Then: it should be an empty string
  assert!(qualifier.is_empty());
}

#[test]
fn confirmation_qualifier_is_string_alias() {
  // Given: a ConfirmationQualifier constructed from a string
  let qualifier: ConfirmationQualifier = alloc::string::String::from("topic-A");

  // Then: it should behave as a String
  assert_eq!(qualifier.as_str(), "topic-A");
}

#[test]
fn no_qualifier_equals_empty_string() {
  // Given: the NO_QUALIFIER constant and an empty string
  let empty: ConfirmationQualifier = alloc::string::String::from("");

  // Then: they should be equal
  assert_eq!(NO_QUALIFIER, empty);
}
