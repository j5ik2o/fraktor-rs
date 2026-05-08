use core::{any::TypeId, time::Duration};

use fraktor_actor_core_rs::core::kernel::actor::{
  error::ActorError,
  supervision::{RestartLimit, SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
};

use crate::dsl::FailureHandler;

struct MyError;

#[test]
fn should_store_type_id_and_name() {
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(5),
    Duration::from_secs(1),
    |error| match error {
      | ActorError::Recoverable(_) => SupervisorDirective::Restart,
      | ActorError::Fatal(_) => SupervisorDirective::Stop,
      | ActorError::Escalate(_) => SupervisorDirective::Escalate,
    },
  );
  let handler = FailureHandler::new::<MyError>(strategy);

  assert_eq!(handler.type_id(), TypeId::of::<MyError>());
  assert!(handler.type_name().contains("MyError"));
}
