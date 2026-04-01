use crate::core::kernel::actor::supervision::BackoffSupervisorCommand;

#[test]
fn get_current_child_variant_is_constructible() {
  // Given/When: constructing the GetCurrentChild variant
  let command = BackoffSupervisorCommand::GetCurrentChild;

  // Then: it matches the expected variant
  assert!(matches!(command, BackoffSupervisorCommand::GetCurrentChild));
}

#[test]
fn reset_variant_is_constructible() {
  // Given/When: constructing the Reset variant
  let command = BackoffSupervisorCommand::Reset;

  // Then: it matches the expected variant
  assert!(matches!(command, BackoffSupervisorCommand::Reset));
}

#[test]
fn get_restart_count_variant_is_constructible() {
  // Given/When: constructing the GetRestartCount variant
  let command = BackoffSupervisorCommand::GetRestartCount;

  // Then: it matches the expected variant
  assert!(matches!(command, BackoffSupervisorCommand::GetRestartCount));
}

#[test]
fn command_variants_are_distinct() {
  // Given: all three command variants
  let get_child = BackoffSupervisorCommand::GetCurrentChild;
  let reset = BackoffSupervisorCommand::Reset;
  let get_count = BackoffSupervisorCommand::GetRestartCount;

  // Then: each variant does not match the others
  assert!(!matches!(get_child, BackoffSupervisorCommand::Reset));
  assert!(!matches!(get_child, BackoffSupervisorCommand::GetRestartCount));
  assert!(!matches!(reset, BackoffSupervisorCommand::GetCurrentChild));
  assert!(!matches!(reset, BackoffSupervisorCommand::GetRestartCount));
  assert!(!matches!(get_count, BackoffSupervisorCommand::GetCurrentChild));
  assert!(!matches!(get_count, BackoffSupervisorCommand::Reset));
}

#[test]
fn command_clone_preserves_variant() {
  // Given: a command variant
  let original = BackoffSupervisorCommand::GetCurrentChild;

  // When: cloned
  let cloned = original.clone();

  // Then: clone matches the original variant
  assert!(matches!(cloned, BackoffSupervisorCommand::GetCurrentChild));
}

#[test]
fn command_debug_format_is_non_empty() {
  // Given: a command variant
  let command = BackoffSupervisorCommand::Reset;

  // When: formatted with Debug
  let debug_str = alloc::format!("{:?}", command);

  // Then: the output is non-empty
  assert!(!debug_str.is_empty());
}
