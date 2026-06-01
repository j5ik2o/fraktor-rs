use crate::membership::NodeStatus;

#[test]
fn exiting_is_not_active() {
  assert!(!NodeStatus::Exiting.is_active());
}

#[test]
fn active_statuses_include_weakly_up_as_provisional_member() {
  assert!(NodeStatus::Joining.is_active());
  assert!(NodeStatus::WeaklyUp.is_active());
  assert!(NodeStatus::Up.is_active());
  assert!(NodeStatus::Suspect.is_active());
  assert!(!NodeStatus::PreparingForShutdown.is_active());
  assert!(!NodeStatus::ReadyForShutdown.is_active());
  assert!(!NodeStatus::Leaving.is_active());
  assert!(!NodeStatus::Removed.is_active());
  assert!(!NodeStatus::Dead.is_active());
}

#[test]
fn weakly_up_is_provisional() {
  assert!(NodeStatus::WeaklyUp.is_provisional());
  assert!(!NodeStatus::Joining.is_provisional());
  assert!(!NodeStatus::Up.is_provisional());
}
