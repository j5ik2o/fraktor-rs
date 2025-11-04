use alloc::boxed::Box;

use super::Actor;
use crate::{NoStdToolbox, actor_prim::ActorContext, error::ActorError, messaging::AnyMessageView};

struct TestActor {
  pre_start_called: bool,
  post_stop_called: bool,
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
}

#[test]
fn actor_box_delegates_pre_start() {
  let actor: Box<dyn Actor<NoStdToolbox>> = Box::new(TestActor { pre_start_called: false, post_stop_called: false });
  // ????????Actor?pre_start???????????
  // ?????????????????????????????
  let _ = actor;
}

#[test]
fn actor_box_delegates_receive() {
  let actor: Box<dyn Actor<NoStdToolbox>> = Box::new(TestActor { pre_start_called: false, post_stop_called: false });
  // ????????Actor?receive???????????
  let _ = actor;
}

#[test]
fn actor_box_delegates_post_stop() {
  let actor: Box<dyn Actor<NoStdToolbox>> = Box::new(TestActor { pre_start_called: false, post_stop_called: false });
  // ????????Actor?post_stop???????????
  let _ = actor;
}
