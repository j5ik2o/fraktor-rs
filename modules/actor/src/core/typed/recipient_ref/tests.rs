use alloc::{string::String, vec::Vec};
use core::hint::spin_loop;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex, SharedAccess};

use super::RecipientRef;
use crate::core::{
  kernel::{
    actor::{
      Actor, ActorCell, ActorContext, Pid,
      actor_ref::{ActorRef, ActorRefSender, SendOutcome},
      error::{ActorError, SendError},
      messaging::{AnyMessage, AnyMessageView},
      props::Props,
    },
    system::ActorSystem,
  },
  typed::TypedActorRef,
};

struct ProbeActor {
  received: ArcShared<NoStdMutex<Vec<u32>>>,
}

impl ProbeActor {
  fn new(received: ArcShared<NoStdMutex<Vec<u32>>>) -> Self {
    Self { received }
  }
}

impl Actor for ProbeActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(value) = message.downcast_ref::<u32>() {
      self.received.lock().push(*value);
    } else if let Some(request) = message.downcast_ref::<EchoRequest>() {
      let mut reply_to = request.reply_to.clone();
      reply_to.tell(AnyMessage::new(request.value));
    }
    Ok(())
  }
}

#[derive(Clone)]
struct EchoRequest {
  value:    u32,
  reply_to: ActorRef,
}

struct FailingSender;

impl ActorRefSender for FailingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::closed(message))
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

fn send_via_recipient<R>(recipient: &mut R, message: u32)
where
  R: RecipientRef<u32>, {
  recipient.tell(message);
}

#[test]
fn recipient_ref_is_implemented_for_typed_actor_ref() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let received = received.clone();
    move || ProbeActor::new(received.clone())
  });
  let cell = register_cell(&system, pid, "typed", &props);
  let mut recipient = TypedActorRef::<u32>::from_untyped(cell.actor_ref());

  send_via_recipient(&mut recipient, 7);
  wait_until(|| received.lock().as_slice() == [7]);
}

#[test]
fn recipient_ref_is_implemented_for_untyped_actor_ref() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let received = received.clone();
    move || ProbeActor::new(received.clone())
  });
  let cell = register_cell(&system, pid, "untyped", &props);
  let mut recipient = cell.actor_ref();

  send_via_recipient(&mut recipient, 9);
  wait_until(|| received.lock().as_slice() == [9]);
}

#[test]
fn typed_recipient_ref_supports_ask() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new()))));
  let cell = register_cell(&system, pid, "typed-ask", &props);
  let mut recipient = TypedActorRef::<EchoRequest>::from_untyped(cell.actor_ref());

  let response = RecipientRef::ask::<u32, _>(&mut recipient, |reply_to| EchoRequest {
    value:    41,
    reply_to: reply_to.into_untyped(),
  });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  assert_eq!(future.try_take().expect("ready").expect("ok"), 41);
}

#[test]
fn untyped_recipient_ref_supports_ask() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new()))));
  let cell = register_cell(&system, pid, "untyped-ask", &props);
  let mut recipient = cell.actor_ref();

  let response = RecipientRef::ask::<u32, _>(&mut recipient, |reply_to| EchoRequest { value: 99, reply_to });
  let future = response.future().clone();
  wait_until(|| future.with_read(|inner| inner.is_ready()));
  let result = future.with_write(|inner| inner.try_take()).expect("ready");
  let reply = result.expect("ask ok");
  assert_eq!(reply.payload().downcast_ref::<u32>(), Some(&99));
}

#[test]
fn untyped_recipient_ref_ask_preserves_send_failure_semantics() {
  let mut recipient = ActorRef::null();

  let response = RecipientRef::ask::<u32, _>(&mut recipient, |reply_to| EchoRequest { value: 9, reply_to });
  let result = response.future().with_write(|inner| inner.try_take()).expect("future should be ready");

  assert!(matches!(result, Err(crate::core::kernel::actor::messaging::AskError::SendFailed(_))));
}

/// `TypedActorRef::tell` returns `()` (fire-and-forget, Pekko-compatible).
/// `try_tell` no longer exists on TypedActorRef.
#[test]
fn typed_actor_ref_tell_returns_unit() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let received = received.clone();
    move || ProbeActor::new(received.clone())
  });
  let cell = register_cell(&system, pid, "typed-tell", &props);
  let mut recipient = TypedActorRef::<u32>::from_untyped(cell.actor_ref());

  // Type constraint: tell MUST return ()
  recipient.tell(42);
  wait_until(|| received.lock().as_slice() == [42]);
}

/// `TypedActorRef::tell` on a failing sender does not panic.
#[test]
fn typed_actor_ref_tell_on_failing_sender_does_not_panic() {
  let mut recipient = TypedActorRef::<u32>::from_untyped(ActorRef::new(Pid::new(77, 1), FailingSender));

  // tell is fire-and-forget: no Result, no panic
  recipient.tell(1);
}
