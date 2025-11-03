use alloc::string::ToString;

use cellactor_utils_core_rs::sync::ArcShared;

use super::ActorCell;
use crate::{AnyMessageView, actor::Actor, actor_context::ActorContext, actor_error::ActorError, pid::Pid};

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
fn actor_cell_holds_components() {
  let system = ArcShared::new(crate::SystemState::<crate::NoStdToolbox>::new());
  let props = crate::props_struct::Props::<crate::NoStdToolbox>::from_fn(|| ProbeActor);
  let cell = ActorCell::create(system, Pid::new(1, 0), None, "worker".to_string(), &props);

  assert_eq!(cell.pid(), Pid::new(1, 0));
  assert_eq!(cell.name(), "worker");
  assert!(cell.parent().is_none());
  assert_eq!(cell.mailbox().system_len(), 0);
  assert_eq!(cell.dispatcher().mailbox().system_len(), 0);
}
