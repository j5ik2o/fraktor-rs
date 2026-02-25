use crate::core::membership::NodeStatus;

#[test]
fn exiting_is_not_active() {
  assert!(!NodeStatus::Exiting.is_active());
}

#[test]
fn active_statuses_remain_joining_up_and_suspect() {
  assert!(NodeStatus::Joining.is_active());
  assert!(NodeStatus::Up.is_active());
  assert!(NodeStatus::Suspect.is_active());
  assert!(!NodeStatus::Leaving.is_active());
  assert!(!NodeStatus::Removed.is_active());
  assert!(!NodeStatus::Dead.is_active());
}
