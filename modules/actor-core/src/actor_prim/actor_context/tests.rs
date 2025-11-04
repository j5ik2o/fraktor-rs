use super::ActorContext;
use crate::{NoStdToolbox, actor_prim::Actor, props::PropsGeneric, system::ActorSystemGeneric};

struct TestActor;

impl Actor<NoStdToolbox> for TestActor {
  fn receive(
    &mut self,
    _context: &mut crate::actor_prim::ActorContext<'_, NoStdToolbox>,
    _message: crate::messaging::AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), crate::error::ActorError> {
    Ok(())
  }
}

#[test]
fn actor_context_new() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  assert_eq!(context.pid(), pid);
}

#[test]
fn actor_context_system() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  let retrieved_system = context.system();
  let _ = retrieved_system;
}

#[test]
fn actor_context_pid() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  assert_eq!(context.pid(), pid);
}

#[test]
fn actor_context_reply_to_initially_none() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  assert!(context.reply_to().is_none());
}

#[test]
fn actor_context_set_and_clear_reply_to() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);

  assert!(context.reply_to().is_none());

  context.clear_reply_to();
  assert!(context.reply_to().is_none());
}

#[test]
fn actor_context_reply_without_reply_to() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);

  let result = context.reply(crate::messaging::AnyMessageGeneric::new(42_u32));
  assert!(result.is_err());
}

#[test]
fn actor_context_children() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);

  let children = context.children();
  assert_eq!(children.len(), 0);
}

#[test]
fn actor_context_spawn_child_with_invalid_parent() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  let props = PropsGeneric::from_fn(|| TestActor);

  let result = context.spawn_child(&props);
  assert!(result.is_err());
}

#[test]
fn actor_context_log() {
  use alloc::string::String;

  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);

  context.log(crate::logging::LogLevel::Info, String::from("test message"));
  context.log(crate::logging::LogLevel::Error, String::from("error message"));
}
