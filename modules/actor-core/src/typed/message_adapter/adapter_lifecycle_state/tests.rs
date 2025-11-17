use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::{
  NoStdToolbox, actor_prim::Pid, system::SystemStateGeneric,
  typed::message_adapter::adapter_lifecycle_state::AdapterLifecycleState,
};

#[test]
fn lifecycle_state_reports_alive_flag() {
  let system = ArcShared::new(SystemStateGeneric::new());
  let state = AdapterLifecycleState::<NoStdToolbox>::new(system, Pid::new(1, 0));
  assert!(state.is_alive());
  state.mark_stopped();
  assert!(!state.is_alive());
}
