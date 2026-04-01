use alloc::format;

use crate::core::kernel::routing::{Routee, RouterCommand};

#[test]
fn get_routees_variant_is_constructible() {
  // Given/When
  let cmd = RouterCommand::GetRoutees;

  // Then
  assert!(matches!(cmd, RouterCommand::GetRoutees));
}

#[test]
fn add_routee_variant_is_constructible() {
  // Given
  let routee = Routee::NoRoutee;

  // When
  let cmd = RouterCommand::AddRoutee(routee);

  // Then
  assert!(matches!(cmd, RouterCommand::AddRoutee(Routee::NoRoutee)));
}

#[test]
fn remove_routee_variant_is_constructible() {
  // Given
  let routee = Routee::NoRoutee;

  // When
  let cmd = RouterCommand::RemoveRoutee(routee);

  // Then
  assert!(matches!(cmd, RouterCommand::RemoveRoutee(Routee::NoRoutee)));
}

#[test]
fn adjust_pool_size_variant_is_constructible() {
  // Given/When
  let cmd = RouterCommand::AdjustPoolSize(5);

  // Then
  assert!(matches!(cmd, RouterCommand::AdjustPoolSize(5)));
}

#[test]
fn adjust_pool_size_negative_value() {
  // Given/When
  let cmd = RouterCommand::AdjustPoolSize(-3);

  // Then
  assert!(matches!(cmd, RouterCommand::AdjustPoolSize(-3)));
}

#[test]
fn command_variants_are_distinct() {
  // Given
  let get = RouterCommand::GetRoutees;
  let add = RouterCommand::AddRoutee(Routee::NoRoutee);
  let remove = RouterCommand::RemoveRoutee(Routee::NoRoutee);
  let adjust = RouterCommand::AdjustPoolSize(1);

  // Then
  assert!(matches!(get, RouterCommand::GetRoutees));
  assert!(!matches!(get, RouterCommand::AddRoutee(_)));

  assert!(matches!(add, RouterCommand::AddRoutee(_)));
  assert!(!matches!(add, RouterCommand::RemoveRoutee(_)));

  assert!(matches!(remove, RouterCommand::RemoveRoutee(_)));
  assert!(!matches!(remove, RouterCommand::AdjustPoolSize(_)));

  assert!(matches!(adjust, RouterCommand::AdjustPoolSize(_)));
  assert!(!matches!(adjust, RouterCommand::GetRoutees));
}

#[test]
fn command_clone_preserves_variant() {
  // Given
  let original = RouterCommand::AdjustPoolSize(42);

  // When
  let cloned = original.clone();

  // Then
  assert!(matches!(cloned, RouterCommand::AdjustPoolSize(42)));
}

#[test]
fn command_debug_format_is_non_empty() {
  // Given
  let cmd = RouterCommand::GetRoutees;

  // When
  let debug_str = format!("{:?}", cmd);

  // Then
  assert!(!debug_str.is_empty());
}
