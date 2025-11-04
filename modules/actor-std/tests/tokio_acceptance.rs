#![cfg(feature = "tokio-executor")]

use core::future::Future;
use std::{
  num::NonZeroUsize,
  time::{Duration, Instant},
};

use cellactor_actor_core_rs::{
  error::ActorError,
  lifecycle::LifecycleStage,
  mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  props::{MailboxConfig, SupervisorOptions},
  supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
};
use cellactor_actor_std_rs::{
  actor_prim::{Actor, ActorContext, ActorRef, ChildRef},
  dispatcher::{DispatcherConfig, dispatch_executor::TokioExecutor},
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  messaging::{AnyMessage, AnyMessageView, AskResponse},
  props::Props,
  system::ActorSystem,
};
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::StdMutex;
use tokio::{
  runtime::{Builder, Handle},
  time::{sleep, timeout},
};

const WAIT_TIMEOUT: Duration = Duration::from_secs(2);

#[test]
fn tokio_ping_pong_acceptance() {
  run_with_runtime(|| async {
    let handle = Handle::current();
    let dispatcher = dispatcher_config(&handle);
    let log = ArcShared::new(StdMutex::new(Vec::new()));

    let props = Props::from_fn({
      let dispatcher = dispatcher.clone();
      let log = log.clone();
      move || PingPongGuardian::new(dispatcher.clone(), log.clone())
    })
    .with_dispatcher(dispatcher.clone());

    let system = ActorSystem::new(&props).expect("system");
    let termination = system.when_terminated();

    system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start guardian");

    wait_until_async("ping log", || log.lock().len() == 3).await;
    assert_eq!(log.lock().clone(), vec!["pong-ping-1", "pong-ping-2", "pong-ping-3"]);

    // Ask guardian for a snapshot of the log via reply_to
    let snapshot = await_message(system.user_guardian_ref().ask(AnyMessage::new(ReadLog)).expect("ask log")).await;
    let view = snapshot.as_view();
    let payload = view.downcast_ref::<LogSnapshot>().expect("snapshot");
    assert_eq!(payload.entries, vec!["pong-ping-1", "pong-ping-2", "pong-ping-3"]);

    system.terminate().expect("terminate");
    termination.listener().await;
  });
}

#[test]
fn tokio_mailbox_backpressure_acceptance() {
  run_with_runtime(|| async {
    let handle = Handle::current();
    let dispatcher = dispatcher_config(&handle);
    let child_slot = ArcShared::new(StdMutex::new(None));
    let guardian = Props::from_fn({
      let slot = child_slot.clone();
      move || SilentGuardian::with_child_slot(slot.clone())
    })
    .with_dispatcher(dispatcher.clone());
    let system = ActorSystem::new(&guardian).expect("system");

    let mailbox_policy = MailboxPolicy::bounded(
      NonZeroUsize::new(2).unwrap(),
      MailboxOverflowStrategy::DropNewest,
      Some(NonZeroUsize::new(3).unwrap()),
    );
    let mailbox = MailboxConfig::new(mailbox_policy);
    let deliveries = ArcShared::new(StdMutex::new(Vec::new()));
    let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
    let subscriber_dyn: ArcShared<dyn EventStreamSubscriber> = subscriber_impl.clone();
    let _subscription = system.subscribe_event_stream(&subscriber_dyn);

    let child_props = Props::from_fn({
      let deliveries = deliveries.clone();
      move || RecordingActor::new(deliveries.clone())
    })
    .with_mailbox(mailbox)
    .with_dispatcher(dispatcher.clone());

    let spawn_req = SpawnChildRequest { props: child_props };
    let spawn_resp =
      await_message(system.user_guardian_ref().ask(AnyMessage::new(spawn_req)).expect("ask spawn")).await;
    let child_ref =
      spawn_resp.as_view().downcast_ref::<SpawnChildResponse>().expect("response").child.actor_ref().clone();
    let actor_ref = child_ref;

    actor_ref.tell(AnyMessage::new(Deliver(1))).expect("deliver 1");
    actor_ref.tell(AnyMessage::new(Deliver(2))).expect("deliver 2");
    let overflow = actor_ref.tell(AnyMessage::new(Deliver(3)));
    assert!(matches!(overflow, Err(cellactor_actor_core_rs::error::SendError::Full(_))));

    wait_until_async("deliveries processed", || deliveries.lock().len() == 2).await;
    assert_eq!(deliveries.lock().clone(), vec![1, 2]);

    wait_until_async("mailbox metrics", || {
      subscriber_impl
        .events()
        .iter()
        .any(|event| matches!(event, EventStreamEvent::Mailbox(metrics) if metrics.capacity() == Some(2)))
    })
    .await;
    wait_until_async("deadletter (overflow)", || {
      subscriber_impl.events().iter().any(|event| matches!(event, EventStreamEvent::DeadLetter(_)))
    })
    .await;

    system.terminate().expect("terminate");
    system.when_terminated().listener().await;
  });
}

#[test]
fn tokio_supervision_and_events_acceptance() {
  run_with_runtime(|| async {
    let handle = Handle::current();
    let dispatcher = dispatcher_config(&handle);
    let guardian_props = Props::from_fn({
      let dispatcher = dispatcher.clone();
      move || SupervisorGuardian::new(dispatcher.clone())
    })
    .with_dispatcher(dispatcher.clone());

    let system = ActorSystem::new(&guardian_props).expect("system");
    let termination = system.when_terminated();
    let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
    let subscriber_dyn: ArcShared<dyn EventStreamSubscriber> = subscriber_impl.clone();
    let _subscription = system.subscribe_event_stream(&subscriber_dyn);

    let child_slot = ArcShared::new(StdMutex::new(None));
    system
      .user_guardian_ref()
      .tell(AnyMessage::new(SupervisionStart { slot: child_slot.clone() }))
      .expect("start supervision");

    wait_until_async("supervised child slot", || child_slot.lock().is_some()).await;
    let supervised = child_slot.lock().clone().unwrap();
    let actor_ref = supervised.child.actor_ref().clone();

    supervised.tell(AnyMessage::new(TriggerRecoverable)).expect("trigger recoverable");
    wait_until_async("restart stats", || {
      supervised.log().lock().iter().filter(|entry| **entry == "child_pre_start").count() >= 2
    })
    .await;

    supervised.tell(AnyMessage::new(TriggerFatal)).expect("trigger fatal");

    // アクターが停止したことを、Stoppedライフサイクルイベントで確認
    let stopped_pid = actor_ref.pid();
    wait_until_async("lifecycle stopped", || {
      subscriber_impl.events().iter().any(|event| {
        matches!(event, EventStreamEvent::Lifecycle(lifecycle)
          if lifecycle.stage() == LifecycleStage::Stopped && lifecycle.pid() == stopped_pid)
      })
    })
    .await;

    system.terminate().expect("terminate");
    termination.listener().await;
  });
}

fn dispatcher_config(handle: &Handle) -> DispatcherConfig {
  DispatcherConfig::from_executor(ArcShared::new(TokioExecutor::new(handle.clone())))
}

fn run_with_runtime<F, Fut>(test: F)
where
  F: FnOnce() -> Fut,
  Fut: Future<Output = ()>, {
  let runtime = Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("runtime");
  runtime.block_on(test());
}

async fn wait_until_async<F>(desc: &str, mut predicate: F)
where
  F: FnMut() -> bool, {
  let deadline = Instant::now() + WAIT_TIMEOUT;
  loop {
    if predicate() {
      return;
    }
    if Instant::now() >= deadline {
      panic!("condition '{desc}' not satisfied within {:?}", WAIT_TIMEOUT);
    }
    sleep(Duration::from_millis(2)).await;
  }
}

async fn await_message(response: AskResponse) -> AnyMessage {
  let future = response.future().clone();
  timeout(WAIT_TIMEOUT, future.listener()).await.expect("ask timeout")
}

struct PingPongGuardian {
  dispatcher: DispatcherConfig,
  log:        ArcShared<StdMutex<Vec<String>>>,
}

impl PingPongGuardian {
  fn new(dispatcher: DispatcherConfig, log: ArcShared<StdMutex<Vec<String>>>) -> Self {
    Self { dispatcher, log }
  }
}

impl Actor for PingPongGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let pong_props = Props::from_fn(|| PongActor).with_dispatcher(self.dispatcher.clone());
      let pong = ctx.spawn_child(&pong_props).map_err(|_| ActorError::recoverable("spawn pong"))?;

      let ping_props = Props::from_fn(|| PingActor).with_dispatcher(self.dispatcher.clone());
      let ping = ctx.spawn_child(&ping_props).map_err(|_| ActorError::recoverable("spawn ping"))?;

      let start = StartPing { target: pong.actor_ref().clone(), reply_to: ctx.self_ref(), count: 3 };
      ping.tell(AnyMessage::new(start)).map_err(|_| ActorError::recoverable("start ping"))?;
    } else if let Some(reply) = message.downcast_ref::<PongReply>() {
      self.log.lock().push(reply.text.clone());
    } else if message.downcast_ref::<ReadLog>().is_some() {
      let snapshot = LogSnapshot { entries: self.log.lock().clone() };
      ctx.reply(AnyMessage::new(snapshot)).map_err(|_| ActorError::recoverable("reply"))?;
    }
    Ok(())
  }
}

struct PingActor;

impl Actor for PingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(cmd) = message.downcast_ref::<StartPing>() {
      for index in 0..cmd.count {
        let payload = PingMessage { text: format!("ping-{}", index + 1), reply_to: cmd.reply_to.clone() };
        cmd.target.tell(AnyMessage::new(payload)).map_err(|_| ActorError::recoverable("send"))?;
      }
    }
    Ok(())
  }
}

struct PongActor;

impl Actor for PongActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<PingMessage>() {
      let entry = format!("pong-{}", ping.text);
      ping.reply_to.tell(AnyMessage::new(PongReply { text: entry })).map_err(|_| ActorError::recoverable("reply"))?;
    }
    Ok(())
  }
}

struct RecordingActor {
  deliveries: ArcShared<StdMutex<Vec<u32>>>,
}

impl RecordingActor {
  fn new(deliveries: ArcShared<StdMutex<Vec<u32>>>) -> Self {
    Self { deliveries }
  }
}

impl Actor for RecordingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(deliver) = message.downcast_ref::<Deliver>() {
      self.deliveries.lock().push(deliver.0);
    }
    Ok(())
  }
}

struct SupervisorGuardian {
  dispatcher: DispatcherConfig,
}

impl SupervisorGuardian {
  fn new(dispatcher: DispatcherConfig) -> Self {
    Self { dispatcher }
  }
}

impl Actor for SupervisorGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(start) = message.downcast_ref::<SupervisionStart>() {
      let log = ArcShared::new(StdMutex::new(Vec::new()));
      let child_props = Props::from_fn({
        let log = log.clone();
        move || RestartChild::new(log.clone())
      })
      .with_dispatcher(self.dispatcher.clone())
      .with_supervisor(SupervisorOptions::new(SupervisorStrategy::new(
        SupervisorStrategyKind::OneForOne,
        5,
        Duration::from_secs(1),
        |error| match error {
          | ActorError::Recoverable(_) => SupervisorDirective::Restart,
          | ActorError::Fatal(_) => SupervisorDirective::Stop,
        },
      )));

      let child = ctx.spawn_child(&child_props).map_err(|_| ActorError::recoverable("spawn child"))?;
      *start.slot.lock() = Some(SupervisedChild { child, log });
    }
    Ok(())
  }
}

struct RestartChild {
  log: ArcShared<StdMutex<Vec<&'static str>>>,
}

impl RestartChild {
  fn new(log: ArcShared<StdMutex<Vec<&'static str>>>) -> Self {
    Self { log }
  }
}

impl Actor for RestartChild {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("child_pre_start");
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<TriggerRecoverable>().is_some() {
      self.log.lock().push("child_fail");
      return Err(ActorError::recoverable("recoverable failure"));
    }
    if message.downcast_ref::<TriggerFatal>().is_some() {
      return Err(ActorError::fatal("fatal failure"));
    }
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("child_post_stop");
    Ok(())
  }
}

struct RecordingSubscriber {
  events: ArcShared<StdMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(StdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<EventStreamEvent> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

struct Start;
struct Deliver(u32);
struct StartPing {
  target:   ActorRef,
  reply_to: ActorRef,
  count:    u32,
}
struct PingMessage {
  text:     String,
  reply_to: ActorRef,
}
struct PongReply {
  text: String,
}
struct ReadLog;
struct LogSnapshot {
  entries: Vec<String>,
}
struct SupervisionStart {
  slot: ArcShared<StdMutex<Option<SupervisedChild>>>,
}
#[derive(Clone)]
struct SupervisedChild {
  child: ChildRef,
  log:   ArcShared<StdMutex<Vec<&'static str>>>,
}
struct TriggerRecoverable;
struct TriggerFatal;

struct SilentGuardian {
  child_slot: Option<ArcShared<StdMutex<Option<ChildRef>>>>,
}

impl SilentGuardian {
  fn new() -> Self {
    Self { child_slot: None }
  }

  fn with_child_slot(child_slot: ArcShared<StdMutex<Option<ChildRef>>>) -> Self {
    Self { child_slot: Some(child_slot) }
  }
}

impl Actor for SilentGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(spawn_req) = message.downcast_ref::<SpawnChildRequest>() {
      let child = ctx.spawn_child(&spawn_req.props).map_err(|_| ActorError::recoverable("spawn failed"))?;
      if let Some(slot) = &self.child_slot {
        *slot.lock() = Some(child.clone());
      }
      ctx.reply(AnyMessage::new(SpawnChildResponse { child })).map_err(|_| ActorError::recoverable("reply"))?;
    }
    Ok(())
  }
}

struct SpawnChildRequest {
  props: Props,
}

struct SpawnChildResponse {
  child: ChildRef,
}

impl SupervisedChild {
  fn tell(
    &self,
    message: AnyMessage,
  ) -> Result<(), cellactor_actor_core_rs::error::SendError<cellactor_utils_std_rs::StdToolbox>> {
    self.child.tell(message)
  }

  fn log(&self) -> ArcShared<StdMutex<Vec<&'static str>>> {
    self.log.clone()
  }
}
