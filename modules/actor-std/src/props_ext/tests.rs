use cellactor_actor_core_rs::{Actor, ActorContext, ActorError, AnyMessageView, Props};
use tokio::runtime::Runtime;

use super::TokioPropsExt;

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn with_tokio_dispatcher_uses_provided_handle() {
  let runtime = Runtime::new().expect("failed to create runtime");
  let handle = runtime.handle().clone();
  let props = Props::from_fn(|| NoopActor).with_tokio_dispatcher(handle);
  let _ = props.dispatcher();
}

#[test]
fn try_with_tokio_dispatcher_succeeds_inside_runtime() {
  let runtime = Runtime::new().expect("failed to create runtime");
  runtime.block_on(async {
    let props = Props::from_fn(|| NoopActor);
    let updated = props.try_with_tokio_dispatcher().expect("handle available");
    let _ = updated.dispatcher();
  });
}

#[test]
fn try_with_tokio_dispatcher_fails_outside_runtime() {
  let props = Props::from_fn(|| NoopActor);
  assert!(props.try_with_tokio_dispatcher().is_err());
}
