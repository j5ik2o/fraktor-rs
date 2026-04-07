use alloc::{string::String, vec::Vec};
use core::hint::spin_loop;

use fraktor_utils_core_rs::core::sync::{ArcShared, NoStdMutex};

use super::StashBuffer;
use crate::core::{
  kernel::{
    actor::{
      Actor, ActorCell, ActorContext, Pid,
      error::ActorError,
      messaging::{AnyMessage, AnyMessageView},
      props::Props,
    },
    system::ActorSystem,
  },
  typed::actor::TypedActorContext,
};

struct ProbeActor {
  received: ArcShared<NoStdMutex<Vec<i32>>>,
}

impl ProbeActor {
  fn new(received: ArcShared<NoStdMutex<Vec<i32>>>) -> Self {
    Self { received }
  }
}

impl Actor for ProbeActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(value) = message.downcast_ref::<i32>() {
      self.received.lock().push(*value);
    }
    Ok(())
  }
}

fn register_cell(system: &ActorSystem, pid: Pid, name: &str, props: &Props) -> ArcShared<ActorCell> {
  let cell = ActorCell::create(system.state(), pid, None, String::from(name), props).expect("create actor cell");
  system.state().register_cell(cell.clone());
  cell
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}

#[test]
fn stash_buffer_capacity_matches_constructor() {
  let stash = StashBuffer::<u32>::new(8);
  assert_eq!(stash.capacity(), 8);
}

#[test]
fn stash_buffer_inspects_and_clears_stashed_messages() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new()))));
  let cell = register_cell(&system, pid, "self", &props);

  let mut context = ActorContext::new(&system, pid);
  context.set_current_message(Some(AnyMessage::new(1_i32)));
  context.stash().expect("stash first");
  context.set_current_message(Some(AnyMessage::new(2_i32)));
  context.stash().expect("stash second");
  context.clear_current_message();

  let typed_ctx = TypedActorContext::from_untyped(&mut context, None);
  let stash = StashBuffer::<i32>::new(8);

  assert_eq!(stash.head(&typed_ctx).expect("head"), 1);
  assert!(stash.contains(&typed_ctx, &2).expect("contains"));
  assert!(stash.exists(&typed_ctx, |message| *message == 1).expect("exists"));

  let seen = ArcShared::new(NoStdMutex::new(Vec::new()));
  let seen_clone = seen.clone();
  stash.foreach(&typed_ctx, move |message| seen_clone.lock().push(*message)).expect("foreach");
  assert_eq!(seen.lock().as_slice(), &[1, 2]);

  stash.clear(&typed_ctx).expect("clear");
  assert!(stash.is_empty(&typed_ctx).expect("empty"));
  assert_eq!(cell.stashed_message_len(), 0);
}

#[test]
fn stash_buffer_unstash_requeues_limited_messages_with_wrap() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let received = received.clone();
    move || ProbeActor::new(received.clone())
  });
  let cell = register_cell(&system, pid, "self", &props);

  let mut context = ActorContext::new(&system, pid);
  context.set_current_message(Some(AnyMessage::new(1_i32)));
  context.stash().expect("stash first");
  context.set_current_message(Some(AnyMessage::new(2_i32)));
  context.stash().expect("stash second");
  context.set_current_message(Some(AnyMessage::new(3_i32)));
  context.stash().expect("stash third");
  context.clear_current_message();

  let typed_ctx = TypedActorContext::from_untyped(&mut context, None);
  let stash = StashBuffer::<i32>::new(8);

  let unstashed = stash.unstash(&typed_ctx, 2, |message| message + 10).expect("unstash");
  assert_eq!(unstashed, 2);

  wait_until(|| received.lock().len() == 2);
  assert_eq!(received.lock().as_slice(), &[11, 12]);
  assert_eq!(cell.stashed_message_len(), 1);
  assert_eq!(stash.head(&typed_ctx).expect("remaining head"), 3);
}
