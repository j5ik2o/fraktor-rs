use core::time::Duration;

use crate::{
  core::error::ActorError,
  std::{
    actor::{Actor, ActorContext},
    futures::ActorFutureListener,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    system::ActorSystem,
  },
};

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_, '_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::expect_used)]
async fn quickstart_bootstraps_system_with_tokio_defaults() {
  let props = Props::from_fn(|| GuardianActor);
  let system = ActorSystem::quickstart(&props).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
  tokio::time::sleep(Duration::from_millis(20)).await;

  assert_eq!(system.state().system_name(), "default-system");

  system.terminate().expect("terminate");
  ActorFutureListener::new(system.when_terminated()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::expect_used)]
async fn quickstart_with_applies_config_override() {
  let props = Props::from_fn(|| GuardianActor);
  let system =
    ActorSystem::quickstart_with(&props, |config| config.with_system_name("quickstart-system")).expect("system");

  assert_eq!(system.state().system_name(), "quickstart-system");

  system.terminate().expect("terminate");
  ActorFutureListener::new(system.when_terminated()).await;
}
