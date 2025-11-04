use alloc::boxed::Box;

use super::ActorFactory;
use crate::{
  NoStdToolbox,
  actor_prim::{Actor, ActorContext},
  error::ActorError,
  messaging::AnyMessageView,
};

struct TestActor;

impl Actor<NoStdToolbox> for TestActor {
  fn receive(
    &mut self,
    _context: &mut ActorContext<'_, NoStdToolbox>,
    _message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn actor_factory_closure_creates_actor() {
  let factory: Box<dyn ActorFactory<NoStdToolbox>> = Box::new(|| TestActor);
  let actor = factory.create();
  // Actor????????????actor?None?????????
  let _ = actor;
}

#[test]
fn actor_factory_can_be_called_multiple_times() {
  let factory: Box<dyn ActorFactory<NoStdToolbox>> = Box::new(|| TestActor);
  let actor1 = factory.create();
  let actor2 = factory.create();
  // ?????????????
  let _ = actor1;
  let _ = actor2;
}
