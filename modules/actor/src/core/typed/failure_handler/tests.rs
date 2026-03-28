use core::{any::TypeId, time::Duration};

use crate::core::{
  kernel::supervision::{SupervisorStrategy, SupervisorStrategyKind},
  typed::failure_handler::FailureHandler,
};

struct MyError;

#[test]
fn should_store_type_id_and_name() {
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 5, Duration::from_secs(1), |error| match error {
      | crate::core::kernel::error::ActorError::Recoverable(_) => {
        crate::core::kernel::supervision::SupervisorDirective::Restart
      },
      | crate::core::kernel::error::ActorError::Fatal(_) => crate::core::kernel::supervision::SupervisorDirective::Stop,
    });
  let handler = FailureHandler::new::<MyError>(strategy);

  assert_eq!(handler.type_id(), TypeId::of::<MyError>());
  assert!(handler.type_name().contains("MyError"));
}
