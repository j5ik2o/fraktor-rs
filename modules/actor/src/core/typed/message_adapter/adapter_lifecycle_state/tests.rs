use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  actor_prim::Pid,
  system::{SystemStateGeneric, SystemStateSharedGeneric},
  typed::message_adapter::adapter_lifecycle_state::AdapterLifecycleState,
};

#[test]
fn lifecycle_state_reports_alive_flag() {
  let system = SystemStateSharedGeneric::new(SystemStateGeneric::new());
  let state = AdapterLifecycleState::<NoStdToolbox>::new(system, Pid::new(1, 0));
  assert!(state.is_alive());
  state.mark_stopped();
  assert!(!state.is_alive());
}
