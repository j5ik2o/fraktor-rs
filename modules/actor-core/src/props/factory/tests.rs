use alloc::boxed::Box;

use super::ActorFactory;
use crate::{
    NoStdToolbox,
    actor_prim::{Actor, ActorContextGeneric},
    error::ActorError,
    messaging::AnyMessageViewGeneric,
};

struct TestActor;

impl Actor for TestActor {
  fn receive(
      &mut self,
      _context: &mut ActorContextGeneric<'_, NoStdToolbox>,
      _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn actor_factory_closure_creates_actor() {
  let factory: Box<dyn ActorFactory<NoStdToolbox>> = Box::new(|| TestActor);
  let actor = factory.create();
  let _ = actor;
}

#[test]
fn actor_factory_can_be_called_multiple_times() {
  let factory: Box<dyn ActorFactory<NoStdToolbox>> = Box::new(|| TestActor);
  let actor1 = factory.create();
  let actor2 = factory.create();
  let _ = actor1;
  let _ = actor2;
}
