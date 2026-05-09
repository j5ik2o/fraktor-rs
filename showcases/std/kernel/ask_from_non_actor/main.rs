use core::time::Duration;
use std::{thread, time::Instant};

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::SharedAccess;

struct Question;
struct Answer(u32);

struct Responder;

impl Actor for Responder {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Question>().is_some() {
      let Some(sender) = message.sender() else {
        return Err(ActorError::recoverable("ask sender missing"));
      };
      let mut reply_to = sender.clone();
      reply_to
        .try_tell(AnyMessage::new(Answer(42)))
        .map_err(|error| ActorError::recoverable(format!("ask reply delivery failed: {error:?}")))?;
    }
    Ok(())
  }
}

fn main() {
  let props = Props::from_fn(|| Responder);
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();
  let mut responder = system.user_guardian_ref();

  let response = responder.ask(AnyMessage::new(Question));
  let deadline = Instant::now() + Duration::from_secs(1);
  while !response.future().with_read(|future| future.is_ready()) {
    assert!(Instant::now() < deadline, "kernel ask should complete within 1 second");
    thread::sleep(Duration::from_millis(1));
  }
  let reply = response.future().with_write(|future| future.try_take()).expect("ready").expect("ok");
  let answer = reply.downcast_ref::<Answer>().expect("answer payload");
  println!("received response: {}", answer.0);
  assert_eq!(answer.0, 42);

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
