use alloc::{format, vec};

use crate::routing::{Routee, RouterResponse};

#[test]
fn routees_variant_is_constructible() {
  // Given
  let routees = vec![Routee::NoRoutee, Routee::NoRoutee];

  // When
  let response = RouterResponse::Routees(routees);

  // Then
  match response {
    | RouterResponse::Routees(ref list) => assert_eq!(list.len(), 2),
  }
}

#[test]
fn routees_variant_with_empty_vec() {
  // Given/When
  let response = RouterResponse::Routees(vec![]);

  // Then
  match response {
    | RouterResponse::Routees(ref list) => assert!(list.is_empty()),
  }
}

#[test]
fn response_clone_preserves_value() {
  // Given
  let original = RouterResponse::Routees(vec![Routee::NoRoutee]);

  // When
  let cloned = original.clone();

  // Then
  match cloned {
    | RouterResponse::Routees(ref list) => {
      assert_eq!(list.len(), 1);
      assert!(matches!(list[0], Routee::NoRoutee));
    },
  }
}

#[test]
fn response_debug_format_is_non_empty() {
  // Given
  let response = RouterResponse::Routees(vec![]);

  // When
  let debug_str = format!("{:?}", response);

  // Then
  assert!(!debug_str.is_empty());
}
