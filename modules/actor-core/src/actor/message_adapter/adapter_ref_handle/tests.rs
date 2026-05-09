use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{AdapterLifecycleState, AdapterRefHandle};

#[test]
fn adapter_ref_handle_controls_lifecycle() {
  let lifecycle = ArcShared::new(AdapterLifecycleState::new());
  let handle = AdapterRefHandle::new(7, lifecycle.clone());
  assert_eq!(handle.id(), 7);
  assert!(lifecycle.is_alive());
  handle.stop();
  assert!(!lifecycle.is_alive());
}
