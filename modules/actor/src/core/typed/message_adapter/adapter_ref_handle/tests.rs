use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::typed::message_adapter::{AdapterLifecycleState, AdapterRefHandle};

#[test]
fn adapter_ref_handle_controls_lifecycle() {
  let lifecycle = ArcShared::new(AdapterLifecycleState::<NoStdToolbox>::new());
  let handle = AdapterRefHandle::new(7, lifecycle.clone());
  assert_eq!(handle.id(), 7);
  assert!(lifecycle.is_alive());
  handle.stop();
  assert!(!lifecycle.is_alive());
}
