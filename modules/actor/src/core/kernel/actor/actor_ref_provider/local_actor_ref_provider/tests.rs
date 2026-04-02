use crate::core::kernel::{
  actor::{
    Actor, ActorContext,
    actor_ref_provider::{ActorRefProvider, LocalActorRefProvider},
    error::ActorError,
    messaging::AnyMessageView,
    props::Props,
  },
  system::ActorSystem,
};

struct ProbeActor;

impl Actor for ProbeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn local_actor_ref_provider_exposes_guardians_dead_letters_and_temp_path() {
  let props = Props::from_fn(|| ProbeActor);
  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let config = crate::core::kernel::actor::setup::ActorSystemConfig::default().with_tick_driver(tick_driver);
  let system = ActorSystem::new_with_config(&props, &config).expect("system");
  let provider = LocalActorRefProvider::new_with_state(system.state());

  assert!(provider.root_guardian().is_some());
  assert!(provider.guardian().is_some());
  assert!(provider.system_guardian().is_some());
  assert_eq!(provider.temp_path().to_relative_string(), "/user/temp");

  let mut dead_letters = provider.dead_letters();
  dead_letters.tell(crate::core::kernel::actor::messaging::AnyMessage::new(String::from("probe")));
  assert!(!system.dead_letters().is_empty());
}
