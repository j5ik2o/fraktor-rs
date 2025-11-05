use alloc::boxed::Box;

use super::Actor;
use crate::{
  NoStdToolbox, actor_prim::ActorContext, error::ActorError, messaging::AnyMessageView, system::ActorSystemGeneric,
};

#[derive(Default)]
struct TestActor {
  pre_start_called:     bool,
  post_stop_called:     bool,
  on_terminated_called: bool,
}

impl Actor<NoStdToolbox> for TestActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.pre_start_called = true;
    Ok(())
  }

  fn receive(
    &mut self,
    _ctx: &mut ActorContext<'_, NoStdToolbox>,
    _message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_, NoStdToolbox>) -> Result<(), ActorError> {
    self.post_stop_called = true;
    Ok(())
  }

  fn on_terminated(
    &mut self,
    _ctx: &mut ActorContext<'_, NoStdToolbox>,
    _terminated: crate::actor_prim::Pid,
  ) -> Result<(), ActorError> {
    self.on_terminated_called = true;
    Ok(())
  }
}

#[test]
fn actor_box_delegates_pre_start() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let mut ctx = ActorContext::new(&system, system.allocate_pid());
  let mut actor: Box<dyn Actor<NoStdToolbox>> = Box::new(TestActor::default());
  assert!(actor.pre_start(&mut ctx).is_ok());
}

#[test]
fn actor_box_delegates_receive() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let mut ctx = ActorContext::new(&system, system.allocate_pid());
  let mut actor: Box<dyn Actor<NoStdToolbox>> = Box::new(TestActor::default());
  let message = crate::messaging::AnyMessageView::new(&(), None);
  assert!(actor.receive(&mut ctx, message).is_ok());
}

#[test]
fn actor_box_delegates_post_stop() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let mut ctx = ActorContext::new(&system, system.allocate_pid());
  let mut actor: Box<dyn Actor<NoStdToolbox>> = Box::new(TestActor::default());
  assert!(actor.post_stop(&mut ctx).is_ok());
}

#[test]
fn actor_box_delegates_on_terminated() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor: Box<dyn Actor<NoStdToolbox>> = Box::new(TestActor::default());
  assert!(actor.on_terminated(&mut ctx, pid).is_ok());
}
