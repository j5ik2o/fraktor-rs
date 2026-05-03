use super::NoopGuardianActor;
use crate::core::kernel::{
  actor::{Actor, ActorContext, messaging::AnyMessage},
  system::ActorSystem,
};

#[test]
fn noop_guardian_actor_receive_accepts_messages() {
  let system = ActorSystem::new_empty();
  let mut context = ActorContext::new(&system, system.allocate_pid());
  let mut actor = NoopGuardianActor::new();
  let message = AnyMessage::new(());
  let view = message.as_view();

  assert!(actor.receive(&mut context, view).is_ok());
}
