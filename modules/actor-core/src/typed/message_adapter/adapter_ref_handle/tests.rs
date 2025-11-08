use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox,
  actor_prim::Pid,
  system::SystemStateGeneric,
  typed::message_adapter::{AdapterLifecycleState, AdapterRefHandle, AdapterRefHandleId},
};

#[test]
fn adapter_ref_handle_controls_lifecycle() {
  let system = ArcShared::new(SystemStateGeneric::new());
  let lifecycle = ArcShared::new(AdapterLifecycleState::<NoStdToolbox>::new(system, Pid::new(1, 0)));
  let handle = AdapterRefHandle::new(AdapterRefHandleId::new(7), lifecycle.clone());
  assert_eq!(handle.id().get(), 7);
  assert!(lifecycle.is_alive());
  handle.stop();
  assert!(!lifecycle.is_alive());
}
