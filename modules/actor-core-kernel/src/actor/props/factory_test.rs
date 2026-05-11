use alloc::boxed::Box;

use super::ActorFactory;
use crate::actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView};

struct TestActor;

impl Actor for TestActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn actor_factory_closure_creates_actor() {
  let mut factory: Box<dyn ActorFactory> = Box::new(|| TestActor);
  let actor = factory.create();
  let _ = actor;
}

#[test]
fn actor_factory_can_be_called_multiple_times() {
  let mut factory: Box<dyn ActorFactory> = Box::new(|| TestActor);
  let actor1 = factory.create();
  let actor2 = factory.create();
  let _ = actor1;
  let _ = actor2;
}
