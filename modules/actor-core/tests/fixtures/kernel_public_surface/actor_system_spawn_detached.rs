use fraktor_actor_core_rs::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::AnyMessageView,
    props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn main() {
  let system = ActorSystem::create_with_noop_guardian(ActorSystemConfig::default()).expect("system");
  let props = Props::from_fn(|| NoopActor);
  let _ = system.spawn_detached(&props);
}
