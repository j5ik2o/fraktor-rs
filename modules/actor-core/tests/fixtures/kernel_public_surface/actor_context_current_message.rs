use fraktor_actor_core_rs::core::kernel::{
  actor::{ActorContext, Pid, messaging::AnyMessage, setup::ActorSystemConfig},
  system::ActorSystem,
};

fn main() {
  let system = ActorSystem::create_with_noop_guardian(ActorSystemConfig::default()).expect("system");
  let mut context = ActorContext::new(&system, Pid::new(1, 0));
  context.set_current_message(Some(AnyMessage::new(1_u8)));
}
