use alloc::string::String;
use core::{hint::spin_loop, time::Duration};

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::{
  kernel::actor::{
    error::ActorError,
    scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
    supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  },
  typed::{
    TypedActorRef, TypedActorSystem, TypedProps,
    behavior::Behavior,
    dsl::{AbstractBehavior, Behaviors, StatusReply, TypedAskError},
  },
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
  let mut actor = crate::core::typed::TypedActorRef::<u32>::from_untyped(actor.into_actor_ref());

  actor.tell(1);
  actor.tell(1);
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

#[derive(Clone, Debug)]
enum AnonymousRestartParentMsg {
  CrashChild,
}

#[derive(Clone, Debug)]
enum AnonymousRestartChildMsg {
  Crash,
}

struct AnonymousSpawnCounterBehavior {
  count: ArcShared<NoStdMutex<u32>>,
}

impl AbstractBehavior<u32> for AnonymousSpawnCounterBehavior {
  fn on_message(
    &mut self,
    _ctx: &mut crate::core::typed::actor::TypedActorContext<'_, u32>,
    msg: &u32,
  ) -> Result<Behavior<u32>, ActorError> {
    *self.count.lock() += *msg;
    Ok(Behaviors::same())
  }
}

struct AnonymousRestartCrashBehavior;

impl AbstractBehavior<AnonymousRestartChildMsg> for AnonymousRestartCrashBehavior {
  fn on_message(
    &mut self,
    _ctx: &mut crate::core::typed::actor::TypedActorContext<'_, AnonymousRestartChildMsg>,
    msg: &AnonymousRestartChildMsg,
  ) -> Result<Behavior<AnonymousRestartChildMsg>, ActorError> {
    match msg {
      | AnonymousRestartChildMsg::Crash => Err(ActorError::recoverable("boom")),
    }
  }
}

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
        reply_to.tell(42);
        Ok(Behaviors::same())
      },
      | ResponderMsg::StatusSuccess { reply_to } => {
        let mut reply_to = reply_to.clone();
        reply_to.tell(StatusReply::success(99));
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
  let mut actor = TypedActorRef::<RequesterMsg>::from_untyped(actor.into_actor_ref());

  // Wait for the responder ref to be registered
  wait_until(|| responder_ref_slot.lock().is_some());

  actor.tell(RequesterMsg::DoAsk);
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
        reply_to.tell(StatusReply::success(99));
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
  let mut actor = TypedActorRef::<RequesterMsg>::from_untyped(actor.into_actor_ref());

  wait_until(|| responder_ref_slot.lock().is_some());

  actor.tell(RequesterMsg::DoAskWithStatus);
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
  let mut actor = TypedActorRef::<RequesterMsg>::from_untyped(actor.into_actor_ref());

  wait_until(|| responder_ref_slot.lock().is_some());

  actor.tell(RequesterMsg::DoAsk);
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
        reply_to.tell(10);
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
  let mut actor = TypedActorRef::<RequesterMsg>::from_untyped(actor.into_actor_ref());

  wait_until(|| responder_ref_slot.lock().is_some());

  actor.tell(RequesterMsg::DoAsk);
  // Both asks should deliver 10 each, totaling 20.
  wait_until(|| *total_received.lock() == 20);

  assert_eq!(*total_received.lock(), 20, "both concurrent asks should deliver their responses");
  system.terminate().expect("terminate");
}

#[test]
fn forward_preserves_sender_through_typed_context() {
  use alloc::vec::Vec;

  use crate::core::kernel::{
    actor::{
      ActorContext, Pid,
      actor_ref::{ActorRef, ActorRefSender, NullSender, SendOutcome},
      messaging::AnyMessage,
    },
    system::ActorSystem,
  };

  struct CapturingSender {
    inbox: ArcShared<NoStdMutex<Vec<AnyMessage>>>,
  }

  impl ActorRefSender for CapturingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, crate::core::kernel::actor::error::SendError> {
      self.inbox.lock().push(message);
      Ok(SendOutcome::Delivered)
    }
  }

  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let target_untyped = ActorRef::new(Pid::new(900, 0), CapturingSender { inbox: inbox.clone() });
  let mut target = TypedActorRef::<u32>::from_untyped(target_untyped);

  let original_sender = ActorRef::new(Pid::new(800, 0), NullSender);

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  context.set_sender(Some(original_sender.clone()));

  let mut typed_ctx = crate::core::typed::actor::TypedActorContext::<u32>::from_untyped(&mut context, None);
  typed_ctx.try_forward(&mut target, 42_u32).expect("forward");

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
  let mut context = crate::core::kernel::actor::ActorContext::new(system.as_untyped(), pid);
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
        let mut reply_to = reply_to.clone();
        reply_to.tell(StatusReply::<u32>::error("domain failure reason"));
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
  let mut actor = TypedActorRef::<RequesterMsg>::from_untyped(actor.into_actor_ref());

  wait_until(|| responder_ref_slot.lock().is_some());

  actor.tell(RequesterMsg::DoAskWithStatusError);
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

  use crate::core::kernel::{
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

  use crate::core::kernel::{
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

  use crate::core::kernel::{
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

// --- T1: spawn_anonymous tests ---

#[test]
fn spawn_anonymous_creates_child_actor_that_receives_messages() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<u32>::new(&guardian_props, TickDriverConfig::manual(ManualTestDriver::new())).expect("system");

  let received = ArcShared::new(NoStdMutex::new(0_u32));

  let parent_props = TypedProps::<u32>::from_behavior_factory({
    let received = received.clone();
    move || {
      let received = received.clone();
      Behaviors::setup(move |ctx| {
        // Given: a behavior that records received messages
        let received_inner = received.clone();
        let child_behavior = Behaviors::receive_message(move |_ctx, msg: &u32| {
          *received_inner.lock() = *msg;
          Ok(Behaviors::same())
        });

        // When: spawn_anonymous is called with the behavior
        let child = ctx.spawn_anonymous(&child_behavior).expect("spawn anonymous");

        // Then: the child actor is created and can receive messages
        let mut child_ref = child.into_actor_ref();
        child_ref.tell(42);

        Behaviors::ignore()
      })
    }
  });

  let _actor = system.as_untyped().spawn(parent_props.to_untyped()).expect("spawn parent");
  wait_until(|| *received.lock() == 42);

  assert_eq!(*received.lock(), 42);
  system.terminate().expect("terminate");
}

#[test]
fn spawn_anonymous_child_has_no_explicit_name() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<u32>::new(&guardian_props, TickDriverConfig::manual(ManualTestDriver::new())).expect("system");

  let child_pid_slot: ArcShared<NoStdMutex<Option<crate::core::kernel::actor::Pid>>> =
    ArcShared::new(NoStdMutex::new(None));

  let parent_props = TypedProps::<u32>::from_behavior_factory({
    let child_pid_slot = child_pid_slot.clone();
    move || {
      let child_pid_slot = child_pid_slot.clone();
      Behaviors::setup(move |ctx| {
        // Given: a simple behavior
        let child_behavior = Behaviors::ignore();

        // When: spawn_anonymous is called
        let child = ctx.spawn_anonymous::<u32>(&child_behavior).expect("spawn anonymous");

        // Then: the child has a valid pid (system-assigned)
        child_pid_slot.lock().replace(child.pid());

        Behaviors::ignore()
      })
    }
  });

  let _actor = system.as_untyped().spawn(parent_props.to_untyped()).expect("spawn parent");
  wait_until(|| child_pid_slot.lock().is_some());

  // The child pid should be valid (non-zero sequence)
  let child_pid = child_pid_slot.lock().expect("child pid should be set");
  assert!(child_pid.value() > 0, "anonymous child should have a valid pid");

  system.terminate().expect("terminate");
}

#[test]
fn spawn_anonymous_multiple_children_are_independent() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<u32>::new(&guardian_props, TickDriverConfig::manual(ManualTestDriver::new())).expect("system");

  let count = ArcShared::new(NoStdMutex::new(0_u32));

  let parent_props = TypedProps::<u32>::from_behavior_factory({
    let count = count.clone();
    move || {
      let count = count.clone();
      Behaviors::setup(move |ctx| {
        // Given: two independent anonymous children
        let count1 = count.clone();
        let behavior1 = Behaviors::receive_message(move |_ctx, _msg: &u32| {
          *count1.lock() += 1;
          Ok(Behaviors::same())
        });
        let count2 = count.clone();
        let behavior2 = Behaviors::receive_message(move |_ctx, _msg: &u32| {
          *count2.lock() += 10;
          Ok(Behaviors::same())
        });

        // When: both are spawned anonymously
        let child1 = ctx.spawn_anonymous(&behavior1).expect("spawn anonymous 1");
        let child2 = ctx.spawn_anonymous(&behavior2).expect("spawn anonymous 2");

        // Then: they have different pids
        assert_ne!(child1.pid(), child2.pid());

        // And: they receive messages independently
        let mut ref1 = child1.into_actor_ref();
        let mut ref2 = child2.into_actor_ref();
        ref1.tell(1);
        ref2.tell(1);

        Behaviors::ignore()
      })
    }
  });

  let _actor = system.as_untyped().spawn(parent_props.to_untyped()).expect("spawn parent");
  wait_until(|| *count.lock() == 11);

  assert_eq!(*count.lock(), 11, "both children should receive independently: 1 + 10 = 11");
  system.terminate().expect("terminate");
}

#[test]
fn spawn_anonymous_can_spawn_same_from_abstract_behavior_twice() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<u32>::new(&guardian_props, TickDriverConfig::manual(ManualTestDriver::new())).expect("system");

  let count = ArcShared::new(NoStdMutex::new(0_u32));

  let parent_props = TypedProps::<u32>::from_behavior_factory({
    let count = count.clone();
    move || {
      let count = count.clone();
      Behaviors::setup(move |ctx| {
        let child_behavior = Behaviors::from_abstract({
          let count = count.clone();
          move |_ctx: &mut crate::core::typed::actor::TypedActorContext<'_, u32>| AnonymousSpawnCounterBehavior {
            count: count.clone(),
          }
        });

        let child1 = ctx.spawn_anonymous(&child_behavior).expect("spawn anonymous 1");
        let child2 = ctx.spawn_anonymous(&child_behavior).expect("spawn anonymous 2");

        let mut ref1 = child1.into_actor_ref();
        let mut ref2 = child2.into_actor_ref();
        ref1.tell(1);
        ref2.tell(1);

        Behaviors::ignore()
      })
    }
  });

  let _actor = system.as_untyped().spawn(parent_props.to_untyped()).expect("spawn parent");
  wait_until(|| *count.lock() == 2);

  assert_eq!(*count.lock(), 2, "from_abstract behavior should initialize independently for each anonymous child");
  system.terminate().expect("terminate");
}

#[test]
fn spawn_anonymous_child_restarts_under_supervision() {
  let guardian_props = TypedProps::<AnonymousRestartParentMsg>::from_behavior_factory(Behaviors::ignore);
  let system = TypedActorSystem::<AnonymousRestartParentMsg>::new(
    &guardian_props,
    TickDriverConfig::manual(ManualTestDriver::new()),
  )
  .expect("system");

  let start_count = ArcShared::new(NoStdMutex::new(0_usize));
  let restart_strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 5, Duration::from_secs(1), |_| {
    SupervisorDirective::Restart
  });

  let parent_props = TypedProps::<AnonymousRestartParentMsg>::from_behavior_factory({
    let start_count = start_count.clone();
    let restart_strategy = restart_strategy.clone();
    move || {
      let start_count = start_count.clone();
      let restart_strategy = restart_strategy.clone();
      Behaviors::setup(move |ctx| {
        let start_count = start_count.clone();
        let child_behavior = Behaviors::supervise(Behaviors::from_abstract(move |_ctx| {
          *start_count.lock() += 1;
          AnonymousRestartCrashBehavior
        }))
        .on_failure(restart_strategy.clone());

        let child = ctx.spawn_anonymous(&child_behavior).expect("spawn anonymous");
        let child_ref = child.into_actor_ref();

        Behaviors::receive_message(move |_ctx, msg: &AnonymousRestartParentMsg| match msg {
          | AnonymousRestartParentMsg::CrashChild => {
            let mut child_ref = child_ref.clone();
            child_ref.tell(AnonymousRestartChildMsg::Crash);
            Ok(Behaviors::same())
          },
        })
      })
    }
  });

  let parent = system.as_untyped().spawn(parent_props.to_untyped()).expect("spawn parent");
  let mut parent = TypedActorRef::<AnonymousRestartParentMsg>::from_untyped(parent.into_actor_ref());

  wait_until(|| *start_count.lock() == 1);

  parent.tell(AnonymousRestartParentMsg::CrashChild);

  wait_until(|| *start_count.lock() >= 2);

  system.terminate().expect("terminate");
}
