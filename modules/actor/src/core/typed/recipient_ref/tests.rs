use alloc::{string::String, vec::Vec};
use core::hint::spin_loop;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex, SharedAccess};

use super::RecipientRef;
use crate::core::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::ActorSystem,
  typed::actor::TypedActorRef,
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
      let reply_to = request.reply_to.clone();
      let _: () = reply_to.tell(AnyMessage::new(request.value));
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
  let _: () = recipient.tell(message);
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
  })
  .expect("ask");
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

  let response =
    RecipientRef::ask::<u32, _>(&mut recipient, |reply_to| EchoRequest { value: 99, reply_to }).expect("ask");
  wait_until(|| response.future().with_read(|future| future.is_ready()));
  let result = response.future().with_write(|future| future.try_take()).expect("ready");
  let reply = result.expect("ask ok");
  assert_eq!(reply.payload().downcast_ref::<u32>(), Some(&99));
}

#[test]
fn typed_actor_ref_try_tell_reports_send_error() {
  let recipient = TypedActorRef::<u32>::from_untyped(ActorRef::new(Pid::new(77, 1), FailingSender));

  let result = recipient.try_tell(1);

  assert!(matches!(result, Err(SendError::Closed(_))));
}
