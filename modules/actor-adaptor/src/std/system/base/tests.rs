use fraktor_actor_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  util::futures::ActorFutureListener,
};

use crate::std::{
  dispatch::dispatcher::DispatcherConfig, scheduler::TickDriverConfig as StdTickDriverConfig, system::ActorSystem,
};

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::expect_used)]
async fn new_bootstraps_system_with_tokio_defaults() {
  let props = Props::from_fn(|| GuardianActor);
  let system = ActorSystem::new(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(Start));

  assert_eq!(system.state().system_name(), "default-system");

  system.terminate().expect("terminate");
  ActorFutureListener::new(system.when_terminated()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::expect_used)]
async fn new_with_config_applies_config_override() {
  let props = Props::from_fn(|| GuardianActor);
  let tick_driver = StdTickDriverConfig::default_config();
  let config = ActorSystemConfig::default()
    .with_tick_driver(tick_driver)
    .with_system_name("custom-system")
    .with_default_dispatcher(DispatcherConfig::default_config().into_core());
  let system = ActorSystem::new_with_config(&props, &config).expect("system");

  assert_eq!(system.state().system_name(), "custom-system");

  system.terminate().expect("terminate");
  ActorFutureListener::new(system.when_terminated()).await;
}
