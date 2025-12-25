use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  actor::Pid,
  system::ActorSystem,
  typed::message_adapter::{AdapterLifecycleState, AdapterRefHandle, AdapterRefHandleId},
};

#[test]
fn adapter_ref_handle_controls_lifecycle() {
  let system = ActorSystem::new_empty().state();
  let lifecycle = ArcShared::new(AdapterLifecycleState::<NoStdToolbox>::new(system, Pid::new(1, 0)));
  let handle = AdapterRefHandle::new(AdapterRefHandleId::new(7), lifecycle.clone());
  assert_eq!(handle.id().get(), 7);
  assert!(lifecycle.is_alive());
  handle.stop();
  assert!(!lifecycle.is_alive());
}
