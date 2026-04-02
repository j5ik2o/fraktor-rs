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
fn command_api_surface_is_stable() {
  let commands = [
    BackoffSupervisorCommand::GetCurrentChild,
    BackoffSupervisorCommand::Reset,
    BackoffSupervisorCommand::GetRestartCount,
  ];

  assert_eq!(commands.len(), 3);
}
