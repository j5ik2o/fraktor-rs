use alloc::string::String;
use core::{hint::spin_loop, time::Duration};

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::{
  error::ActorError,
  messaging::AnyMessage,
  scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  typed::{Behaviors, TypedActorSystem, TypedAskError, TypedProps, actor::TypedActorRef, status_reply::StatusReply},
};

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
fn delegate_returns_delegatee_when_behavior_reports_same() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<u32>::new(&guardian_props, TickDriverConfig::manual(ManualTestDriver::new())).expect("system");

  let outer_count = ArcShared::new(NoStdMutex::new(0_usize));
  let inner_count = ArcShared::new(NoStdMutex::new(0_usize));
  let actor_props = TypedProps::<u32>::from_behavior_factory({
    let outer_count = outer_count.clone();
    let inner_count = inner_count.clone();
    move || {
      let outer_count = outer_count.clone();
      let inner_count = inner_count.clone();
      Behaviors::receive_message(move |ctx, message: &u32| {
        *outer_count.lock() += 1;
        let inner_count = inner_count.clone();
        let delegated = Behaviors::receive_message(move |_ctx, inner_message: &u32| {
          if *inner_message > 0 {
            *inner_count.lock() += 1;
          }
          Ok(Behaviors::same())
        });
        ctx.delegate(delegated, message)
      })
    }
  });
  let actor = system.as_untyped().spawn(actor_props.to_untyped()).expect("spawn actor");
  let mut actor = crate::core::typed::actor::TypedActorRef::<u32>::from_untyped(actor.actor_ref().clone());

  let _: () = actor.tell(1);
  let _: () = actor.tell(1);
  wait_until(|| *inner_count.lock() == 2);

  assert_eq!(*outer_count.lock(), 1);
  assert_eq!(*inner_count.lock(), 2);
  system.terminate().expect("terminate");
}

#[derive(Clone, Debug)]
enum ResponderMsg {
  Value { reply_to: TypedActorRef<u32> },
  StatusSuccess { reply_to: TypedActorRef<StatusReply<u32>> },
  FailureStatus { reply_to: TypedActorRef<StatusReply<u32>> },
}

#[derive(Clone, Debug)]
enum RequesterMsg {
  DoAsk,
  DoAskWithStatus,
  DoAskWithStatusError,
  GotResponse(u32),
  GotResponseFailed,
  GotStatusResponse(u32),
  GotStatusError,
  GotStatusErrorReason(String),
}

const ASK_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn ask_sends_request_and_delivers_adapted_response() {
  let guardian_props = TypedProps::<RequesterMsg>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<RequesterMsg>::new(&guardian_props, TickDriverConfig::manual(ManualTestDriver::new()))
      .expect("system");

  let received = ArcShared::new(NoStdMutex::new(0_u32));
  let responder_ref_slot: ArcShared<NoStdMutex<Option<TypedActorRef<ResponderMsg>>>> =
    ArcShared::new(NoStdMutex::new(None));

  let responder_props = TypedProps::<ResponderMsg>::from_behavior_factory(|| {
    Behaviors::receive_message(|_ctx, msg: &ResponderMsg| match msg {
      | ResponderMsg::Value { reply_to } => {
        let mut reply_to = reply_to.clone();
        let _: () = reply_to.tell(42);
        Ok(Behaviors::same())
      },
      | ResponderMsg::StatusSuccess { reply_to } => {
        let mut reply_to = reply_to.clone();
        let _: () = reply_to.tell(StatusReply::success(99));
        Ok(Behaviors::same())
      },
      | _ => Ok(Behaviors::same()),
    })
  });

  let requester_props = TypedProps::<RequesterMsg>::from_behavior_factory({
    let received = received.clone();
    let responder_ref_slot = responder_ref_slot.clone();
    let responder_props = responder_props.clone();
    move || {
      let received = received.clone();
      let responder_ref_slot = responder_ref_slot.clone();
      let responder_props = responder_props.clone();
      Behaviors::setup(move |ctx| {
        let child = ctx.spawn_child(&responder_props).expect("spawn responder");
        responder_ref_slot.lock().replace(child.actor_ref());
        let received = received.clone();
        let responder_ref_slot = responder_ref_slot.clone();
        Behaviors::receive_message(move |ctx, msg: &RequesterMsg| match msg {
          | RequesterMsg::DoAsk => {
            let mut target = responder_ref_slot.lock().clone().expect("responder ref");
            ctx
              .ask(
                &mut target,
                |reply_to| ResponderMsg::Value { reply_to },
                |result| match result {
                  | Ok(value) => RequesterMsg::GotResponse(value),
                  | Err(_) => RequesterMsg::GotResponseFailed,
                },
                ASK_TIMEOUT,
              )
              .map_err(|e| ActorError::recoverable(alloc::format!("ask failed: {e:?}")))?;
            Ok(Behaviors::same())
          },
          | RequesterMsg::GotResponse(value) => {
            *received.lock() = *value;
            Ok(Behaviors::same())
          },
          | _ => Ok(Behaviors::same()),
        })
      })
    }
  });

  let actor = system.as_untyped().spawn(requester_props.to_untyped()).expect("spawn requester");
  let mut actor = TypedActorRef::<RequesterMsg>::from_untyped(actor.actor_ref().clone());

  // Wait for the responder ref to be registered
  wait_until(|| responder_ref_slot.lock().is_some());

  let _: () = actor.tell(RequesterMsg::DoAsk);
  wait_until(|| *received.lock() == 42);

  assert_eq!(*received.lock(), 42);
  system.terminate().expect("terminate");
}

#[test]
fn ask_with_status_sends_request_and_delivers_adapted_success() {
  let guardian_props = TypedProps::<RequesterMsg>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<RequesterMsg>::new(&guardian_props, TickDriverConfig::manual(ManualTestDriver::new()))
      .expect("system");

  let received = ArcShared::new(NoStdMutex::new(0_u32));
  let responder_ref_slot: ArcShared<NoStdMutex<Option<TypedActorRef<ResponderMsg>>>> =
    ArcShared::new(NoStdMutex::new(None));

  let responder_props = TypedProps::<ResponderMsg>::from_behavior_factory(|| {
    Behaviors::receive_message(|_ctx, msg: &ResponderMsg| match msg {
      | ResponderMsg::Value { .. } => Ok(Behaviors::same()),
      | ResponderMsg::StatusSuccess { reply_to } => {
        let mut reply_to = reply_to.clone();
        let _: () = reply_to.tell(StatusReply::success(99));
        Ok(Behaviors::same())
      },
      | _ => Ok(Behaviors::same()),
    })
  });

  let requester_props = TypedProps::<RequesterMsg>::from_behavior_factory({
    let received = received.clone();
    let responder_ref_slot = responder_ref_slot.clone();
    let responder_props = responder_props.clone();
    move || {
      let received = received.clone();
      let responder_ref_slot = responder_ref_slot.clone();
      let responder_props = responder_props.clone();
      Behaviors::setup(move |ctx| {
        let child = ctx.spawn_child(&responder_props).expect("spawn responder");
        responder_ref_slot.lock().replace(child.actor_ref());
        let received = received.clone();
        let responder_ref_slot = responder_ref_slot.clone();
        Behaviors::receive_message(move |ctx, msg: &RequesterMsg| match msg {
          | RequesterMsg::DoAskWithStatus => {
            let mut target = responder_ref_slot.lock().clone().expect("responder ref");
            ctx
              .ask_with_status(
                &mut target,
                |reply_to| ResponderMsg::StatusSuccess { reply_to },
                |result| match result {
                  | Ok(value) => RequesterMsg::GotStatusResponse(value),
                  | Err(_) => RequesterMsg::GotStatusError,
                },
                ASK_TIMEOUT,
              )
              .map_err(|e| ActorError::recoverable(alloc::format!("ask failed: {e:?}")))?;
            Ok(Behaviors::same())
          },
          | RequesterMsg::GotStatusResponse(value) => {
            *received.lock() = *value;
            Ok(Behaviors::same())
          },
          | _ => Ok(Behaviors::same()),
        })
      })
    }
  });

  let actor = system.as_untyped().spawn(requester_props.to_untyped()).expect("spawn requester");
  let mut actor = TypedActorRef::<RequesterMsg>::from_untyped(actor.actor_ref().clone());

  wait_until(|| responder_ref_slot.lock().is_some());

  let _: () = actor.tell(RequesterMsg::DoAskWithStatus);
  wait_until(|| *received.lock() == 99);

  assert_eq!(*received.lock(), 99);
  system.terminate().expect("terminate");
}

#[test]
fn ask_timeout_delivers_error_to_actor() {
  let guardian_props = TypedProps::<RequesterMsg>::from_behavior_factory(Behaviors::ignore);
  let manual = ManualTestDriver::new();
  let system =
    TypedActorSystem::<RequesterMsg>::new(&guardian_props, TickDriverConfig::manual(manual.clone())).expect("system");

  let got_failure = ArcShared::new(NoStdMutex::new(false));
  let responder_ref_slot: ArcShared<NoStdMutex<Option<TypedActorRef<ResponderMsg>>>> =
    ArcShared::new(NoStdMutex::new(None));

  // Responder that never replies
  let responder_props = TypedProps::<ResponderMsg>::from_behavior_factory(|| {
    Behaviors::receive_message(|_ctx, _msg: &ResponderMsg| Ok(Behaviors::same()))
  });

  let requester_props = TypedProps::<RequesterMsg>::from_behavior_factory({
    let got_failure = got_failure.clone();
    let responder_ref_slot = responder_ref_slot.clone();
    let responder_props = responder_props.clone();
    move || {
      let got_failure = got_failure.clone();
      let responder_ref_slot = responder_ref_slot.clone();
      let responder_props = responder_props.clone();
      Behaviors::setup(move |ctx| {
        let child = ctx.spawn_child(&responder_props).expect("spawn responder");
        responder_ref_slot.lock().replace(child.actor_ref());
        let got_failure = got_failure.clone();
        let responder_ref_slot = responder_ref_slot.clone();
        Behaviors::receive_message(move |ctx, msg: &RequesterMsg| match msg {
          | RequesterMsg::DoAsk => {
            let mut target = responder_ref_slot.lock().clone().expect("responder ref");
            ctx
              .ask(
                &mut target,
                |reply_to| ResponderMsg::Value { reply_to },
                |result| match result {
                  | Ok(value) => RequesterMsg::GotResponse(value),
                  | Err(_) => RequesterMsg::GotResponseFailed,
                },
                Duration::ZERO,
              )
              .map_err(|e| ActorError::recoverable(alloc::format!("ask failed: {e:?}")))?;
            Ok(Behaviors::same())
          },
          | RequesterMsg::GotResponseFailed => {
            *got_failure.lock() = true;
            Ok(Behaviors::same())
          },
          | _ => Ok(Behaviors::same()),
        })
      })
    }
  });

  let actor = system.as_untyped().spawn(requester_props.to_untyped()).expect("spawn requester");
  let mut actor = TypedActorRef::<RequesterMsg>::from_untyped(actor.actor_ref().clone());

  wait_until(|| responder_ref_slot.lock().is_some());

  let _: () = actor.tell(RequesterMsg::DoAsk);
  wait_until(|| *got_failure.lock());

  assert!(*got_failure.lock(), "timeout should deliver failure to actor");
  system.terminate().expect("terminate");
}

#[test]
fn ask_concurrent_same_response_type_delivers_both() {
  let guardian_props = TypedProps::<RequesterMsg>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<RequesterMsg>::new(&guardian_props, TickDriverConfig::manual(ManualTestDriver::new()))
      .expect("system");

  let total_received = ArcShared::new(NoStdMutex::new(0_u32));
  let responder_ref_slot: ArcShared<NoStdMutex<Option<TypedActorRef<ResponderMsg>>>> =
    ArcShared::new(NoStdMutex::new(None));

  let responder_props = TypedProps::<ResponderMsg>::from_behavior_factory(|| {
    Behaviors::receive_message(|_ctx, msg: &ResponderMsg| match msg {
      | ResponderMsg::Value { reply_to } => {
        let mut reply_to = reply_to.clone();
        let _: () = reply_to.tell(10);
        Ok(Behaviors::same())
      },
      | _ => Ok(Behaviors::same()),
    })
  });

  let requester_props = TypedProps::<RequesterMsg>::from_behavior_factory({
    let total_received = total_received.clone();
    let responder_ref_slot = responder_ref_slot.clone();
    let responder_props = responder_props.clone();
    move || {
      let total_received = total_received.clone();
      let responder_ref_slot = responder_ref_slot.clone();
      let responder_props = responder_props.clone();
      Behaviors::setup(move |ctx| {
        let child = ctx.spawn_child(&responder_props).expect("spawn responder");
        responder_ref_slot.lock().replace(child.actor_ref());
        let total_received = total_received.clone();
        let responder_ref_slot = responder_ref_slot.clone();
        Behaviors::receive_message(move |ctx, msg: &RequesterMsg| match msg {
          | RequesterMsg::DoAsk => {
            // Fire two concurrent asks with the same response type (u32).
            // With the old message_adapter approach, the second would overwrite the first.
            let mut target1 = responder_ref_slot.lock().clone().expect("responder ref");
            let mut target2 = target1.clone();
            ctx
              .ask(
                &mut target1,
                |reply_to| ResponderMsg::Value { reply_to },
                |result| match result {
                  | Ok(value) => RequesterMsg::GotResponse(value),
                  | Err(_) => RequesterMsg::GotResponseFailed,
                },
                ASK_TIMEOUT,
              )
              .map_err(|e| ActorError::recoverable(alloc::format!("ask1 failed: {e:?}")))?;
            ctx
              .ask(
                &mut target2,
                |reply_to| ResponderMsg::Value { reply_to },
                |result| match result {
                  | Ok(value) => RequesterMsg::GotResponse(value),
                  | Err(_) => RequesterMsg::GotResponseFailed,
                },
                ASK_TIMEOUT,
              )
              .map_err(|e| ActorError::recoverable(alloc::format!("ask2 failed: {e:?}")))?;
            Ok(Behaviors::same())
          },
          | RequesterMsg::GotResponse(value) => {
            *total_received.lock() += *value;
            Ok(Behaviors::same())
          },
          | _ => Ok(Behaviors::same()),
        })
      })
    }
  });

  let actor = system.as_untyped().spawn(requester_props.to_untyped()).expect("spawn requester");
  let mut actor = TypedActorRef::<RequesterMsg>::from_untyped(actor.actor_ref().clone());

  wait_until(|| responder_ref_slot.lock().is_some());

  let _: () = actor.tell(RequesterMsg::DoAsk);
  // Both asks should deliver 10 each, totaling 20.
  wait_until(|| *total_received.lock() == 20);

  assert_eq!(*total_received.lock(), 20, "both concurrent asks should deliver their responses");
  system.terminate().expect("terminate");
}

#[test]
fn forward_preserves_sender_through_typed_context() {
  use alloc::vec::Vec;

  use crate::core::{
    actor::{
      ActorContext, Pid,
      actor_ref::{ActorRef, ActorRefSender, NullSender, SendOutcome},
    },
    messaging::AnyMessage,
    system::ActorSystem,
  };

  struct CapturingSender {
    inbox: ArcShared<NoStdMutex<Vec<AnyMessage>>>,
  }

  impl ActorRefSender for CapturingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, crate::core::error::SendError> {
      self.inbox.lock().push(message);
      Ok(SendOutcome::Delivered)
    }
  }

  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let target_untyped = ActorRef::new(Pid::new(900, 0), CapturingSender { inbox: inbox.clone() });
  let target = TypedActorRef::<u32>::from_untyped(target_untyped);

  let original_sender = ActorRef::new(Pid::new(800, 0), NullSender);

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  context.set_sender(Some(original_sender.clone()));

  let typed_ctx = crate::core::typed::actor::TypedActorContext::<u32>::from_untyped(&mut context, None);
  typed_ctx.forward(&target, 42_u32).expect("forward should succeed");

  let captured = inbox.lock();
  assert_eq!(captured.len(), 1);
  let forwarded = &captured[0];
  assert_eq!(
    forwarded.sender().expect("sender preserved").pid(),
    original_sender.pid(),
    "typed forward should preserve the original sender"
  );
}

#[test]
fn schedule_once_registers_command_in_scheduler() {
  use fraktor_utils_rs::core::sync::SharedAccess;

  let manual = ManualTestDriver::new();
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system = TypedActorSystem::<u32>::new(&guardian_props, TickDriverConfig::manual(manual.clone())).expect("system");

  // Access the scheduler via the system to verify schedule_once registers a command.
  let scheduler_shared = system.as_untyped().scheduler();
  let initial_pending = scheduler_shared.with_read(|s| s.dump().jobs().len());

  // Create a typed actor context with the system and call schedule_once.
  let pid = system.as_untyped().allocate_pid();
  let mut context = crate::core::actor::ActorContext::new(system.as_untyped(), pid);
  let typed_ctx = crate::core::typed::actor::TypedActorContext::<u32>::from_untyped(&mut context, None);

  let target = system.user_guardian_ref();
  let handle = typed_ctx.schedule_once(Duration::from_millis(100), target, 42_u32);
  assert!(handle.is_ok(), "schedule_once should succeed");

  // Verify that a new pending job was registered.
  let after_pending = scheduler_shared.with_read(|s| s.dump().jobs().len());
  assert_eq!(after_pending, initial_pending + 1, "a new scheduler job should be registered");

  system.terminate().expect("terminate");
}

#[test]
fn ask_with_status_error_preserves_failure_reason() {
  let guardian_props = TypedProps::<RequesterMsg>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<RequesterMsg>::new(&guardian_props, TickDriverConfig::manual(ManualTestDriver::new()))
      .expect("system");

  let captured_reason = ArcShared::new(NoStdMutex::new(String::new()));
  let responder_ref_slot: ArcShared<NoStdMutex<Option<TypedActorRef<ResponderMsg>>>> =
    ArcShared::new(NoStdMutex::new(None));

  let responder_props = TypedProps::<ResponderMsg>::from_behavior_factory(|| {
    Behaviors::receive_message(|_ctx, msg: &ResponderMsg| match msg {
      | ResponderMsg::FailureStatus { reply_to } => {
        let reply_to = reply_to.clone();
        reply_to
          .as_untyped()
          .try_tell(AnyMessage::new(StatusReply::<u32>::error("domain failure reason")))
          .map_err(|e| ActorError::from_send_error(&e))?;
        Ok(Behaviors::same())
      },
      | _ => Ok(Behaviors::same()),
    })
  });

  let requester_props = TypedProps::<RequesterMsg>::from_behavior_factory({
    let captured_reason = captured_reason.clone();
    let responder_ref_slot = responder_ref_slot.clone();
    let responder_props = responder_props.clone();
    move || {
      let captured_reason = captured_reason.clone();
      let responder_ref_slot = responder_ref_slot.clone();
      let responder_props = responder_props.clone();
      Behaviors::setup(move |ctx| {
        let child = ctx.spawn_child(&responder_props).expect("spawn responder");
        responder_ref_slot.lock().replace(child.actor_ref());
        let captured_reason = captured_reason.clone();
        let responder_ref_slot = responder_ref_slot.clone();
        Behaviors::receive_message(move |ctx, msg: &RequesterMsg| match msg {
          | RequesterMsg::DoAskWithStatusError => {
            let mut target = responder_ref_slot.lock().clone().expect("responder ref");
            ctx
              .ask_with_status(
                &mut target,
                |reply_to| ResponderMsg::FailureStatus { reply_to },
                |result| match result {
                  | Ok(value) => RequesterMsg::GotStatusResponse(value),
                  | Err(TypedAskError::StatusError(status_err)) => {
                    RequesterMsg::GotStatusErrorReason(String::from(status_err.message()))
                  },
                  | Err(_) => RequesterMsg::GotStatusError,
                },
                ASK_TIMEOUT,
              )
              .map_err(|e| ActorError::recoverable(alloc::format!("ask failed: {e:?}")))?;
            Ok(Behaviors::same())
          },
          | RequesterMsg::GotStatusErrorReason(reason) => {
            *captured_reason.lock() = reason.clone();
            Ok(Behaviors::same())
          },
          | _ => Ok(Behaviors::same()),
        })
      })
    }
  });

  let actor = system.as_untyped().spawn(requester_props.to_untyped()).expect("spawn requester");
  let mut actor = TypedActorRef::<RequesterMsg>::from_untyped(actor.actor_ref().clone());

  wait_until(|| responder_ref_slot.lock().is_some());

  let _: () = actor.tell(RequesterMsg::DoAskWithStatusError);
  wait_until(|| !captured_reason.lock().is_empty());

  assert_eq!(
    captured_reason.lock().as_str(),
    "domain failure reason",
    "StatusReply::Error reason must be preserved as StatusError variant"
  );
  system.terminate().expect("terminate");
}

#[test]
fn typed_props_with_tags_are_readable_via_typed_context() {
  use alloc::string::String;

  use crate::core::{
    actor::{ActorCell, ActorContext},
    system::ActorSystem,
  };

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore).with_tags(["observer", "critical"]);

  let cell =
    ActorCell::create(system.state(), pid, None, String::from("tagged-typed-actor"), props.to_untyped()).expect("cell");
  system.state().register_cell(cell);
  let mut context = ActorContext::new(&system, pid);

  let typed_ctx = crate::core::typed::actor::TypedActorContext::<u32>::from_untyped(&mut context, None);
  let tags = typed_ctx.tags();
  assert_eq!(tags.len(), 2);
  assert!(tags.contains("observer"));
  assert!(tags.contains("critical"));
}

#[test]
fn typed_props_with_tag_adds_single_tag_readable_via_typed_context() {
  use alloc::string::String;

  use crate::core::{
    actor::{ActorCell, ActorContext},
    system::ActorSystem,
  };

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore).with_tag("alpha").with_tag("beta");

  let cell = ActorCell::create(system.state(), pid, None, String::from("single-tagged-actor"), props.to_untyped())
    .expect("cell");
  system.state().register_cell(cell);
  let mut context = ActorContext::new(&system, pid);

  let typed_ctx = crate::core::typed::actor::TypedActorContext::<u32>::from_untyped(&mut context, None);
  let tags = typed_ctx.tags();
  assert_eq!(tags.len(), 2);
  assert!(tags.contains("alpha"));
  assert!(tags.contains("beta"));
}

#[test]
fn typed_props_without_tags_returns_empty_via_typed_context() {
  use alloc::string::String;

  use crate::core::{
    actor::{ActorCell, ActorContext},
    system::ActorSystem,
  };

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);

  let cell =
    ActorCell::create(system.state(), pid, None, String::from("plain-typed-actor"), props.to_untyped()).expect("cell");
  system.state().register_cell(cell);
  let mut context = ActorContext::new(&system, pid);

  let typed_ctx = crate::core::typed::actor::TypedActorContext::<u32>::from_untyped(&mut context, None);
  assert!(typed_ctx.tags().is_empty());
}
