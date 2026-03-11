use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::{
  core::{
    actor::ActorContext,
    system::ActorSystem,
    typed::{BehaviorSignal, Behaviors as CoreBehaviors, actor::TypedActorContext},
  },
  std::typed::Behaviors,
};

#[test]
fn log_messages_delegates_to_inner_behavior() {
  let inner_received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let inner_received_clone = inner_received.clone();

  let mut behavior = Behaviors::log_messages(move || {
    let received = inner_received_clone.clone();
    CoreBehaviors::receive_message(move |_ctx, msg: &u32| {
      received.lock().push(*msg);
      Ok(CoreBehaviors::same())
    })
  });

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
  inner.handle_message(&mut typed_ctx, &77u32).expect("message");

  let captured = inner_received.lock();
  assert_eq!(captured.len(), 1, "inner behavior should have received the message");
  assert_eq!(captured[0], 77);
}

#[test]
fn receive_message_handles_message() {
  let received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let received_clone = received.clone();
  let captured_pid = ArcShared::new(NoStdMutex::new(0u64));
  let captured_pid_clone = captured_pid.clone();

  let mut behavior = Behaviors::receive_message(move |ctx, msg: &u32| {
    received_clone.lock().push(*msg);
    *captured_pid_clone.lock() = ctx.pid().value();
    Ok(CoreBehaviors::same())
  });

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  behavior.handle_message(&mut typed_ctx, &11u32).expect("message");

  assert_eq!(received.lock().as_slice(), &[11]);
  assert_eq!(*captured_pid.lock(), typed_ctx.pid().value());
}
