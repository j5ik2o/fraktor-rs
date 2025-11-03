use alloc::string::ToString;

use cellactor_utils_core_rs::sync::ArcShared;

use super::SystemState;
use crate::{AnyMessageView, actor::Actor, actor_context::ActorContext, actor_error::ActorError};

struct ProbeActor;

impl Actor<crate::NoStdToolbox> for ProbeActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContext<'_, crate::NoStdToolbox>,
    _message: AnyMessageView<'_, crate::NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn registers_and_fetches_cells() {
  let state = ArcShared::new(SystemState::<crate::NoStdToolbox>::new());
  let props = crate::props_struct::Props::<crate::NoStdToolbox>::from_fn(|| ProbeActor);
  let pid = state.allocate_pid();
  let cell = crate::ActorCell::create(state.clone(), pid, None, "worker".to_string(), &props);
  state.register_cell(cell.clone());
  assert!(state.cell(&pid).is_some());
  state.remove_cell(&pid);
  assert!(state.cell(&pid).is_none());
}
