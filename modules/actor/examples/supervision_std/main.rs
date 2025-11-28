use core::time::Duration as CoreDuration;
use std::{thread, time::Duration};

#[path = "../std_tick_driver_support.rs"]
mod std_tick_driver_support;

use fraktor_actor_rs::{
  core::{
    error::ActorError,
    logging::{LogEvent, LogLevel, LoggerWriter},
    supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  },
  std::{
    actor_prim::{Actor, ActorContext},
    event_stream::{EventStreamEvent, EventStreamSubscriber, EventStreamSubscriberShared, subscriber_handle},
    logging::StdLoggerSubscriber,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    system::ActorSystem,
  },
};

struct Start;
struct Trigger;

struct StdoutLogger;

impl LoggerWriter for StdoutLogger {
  fn write(&mut self, event: &LogEvent) {
    println!("[LOG {:?}] origin={:?} message={}", event.level(), event.origin(), event.message());
  }
}

struct StdLoggerAdapter(StdLoggerSubscriber);

impl StdLoggerAdapter {
  fn new(level: LogLevel, writer: Box<dyn LoggerWriter>) -> Self {
    Self(StdLoggerSubscriber::new(level, writer))
  }
}

impl EventStreamSubscriber for StdLoggerAdapter {
  fn on_event(&mut self, event: &EventStreamEvent) {
    fraktor_actor_rs::core::event_stream::EventStreamSubscriber::on_event(&mut self.0, event);
  }
}

struct LifecyclePrinter;

impl EventStreamSubscriber for LifecyclePrinter {
  fn on_event(&mut self, event: &EventStreamEvent) {
    match event {
      | EventStreamEvent::Lifecycle(lifecycle) => {
        println!("[LIFECYCLE] pid={:?} stage={:?}", lifecycle.pid(), lifecycle.stage());
      },
      | EventStreamEvent::DeadLetter(entry) => {
        println!("[DEAD LETTER] reason={:?} recipient={:?}", entry.reason(), entry.recipient());
      },
      | EventStreamEvent::AdapterFailure(failure) => {
        println!("[ADAPTER FAILURE] pid={:?} reason={:?}", failure.pid(), failure.failure());
      },
      | EventStreamEvent::MailboxPressure(event) => {
        println!("[MAILBOX PRESSURE] pid={:?} usage={:.2}", event.pid(), event.utilization());
      },
      | EventStreamEvent::DispatcherDump(dump) => {
        println!(
          "[DISPATCHER DUMP] pid={:?} user_queue={} system_queue={} running={} suspended={}",
          dump.pid(),
          dump.user_queue_len(),
          dump.system_queue_len(),
          dump.is_running(),
          dump.is_suspended()
        );
      },
      | EventStreamEvent::RemoteAuthority(event) => {
        println!("[REMOTE AUTHORITY] authority={} state={:?}", event.authority(), event.state());
      },
      | EventStreamEvent::Log(_)
      | EventStreamEvent::Mailbox(_)
      | EventStreamEvent::UnhandledMessage(_)
      | EventStreamEvent::Serialization(_)
      | EventStreamEvent::SchedulerTick(_)
      | EventStreamEvent::TickDriver(_)
      | EventStreamEvent::RemotingBackpressure(_)
      | EventStreamEvent::Extension { .. }
      | EventStreamEvent::RemotingLifecycle(_) => {},
    }
  }
}

struct GuardianActor;

impl GuardianActor {
  fn new() -> Self {
    Self
  }
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      ctx.log(LogLevel::Info, "子アクターを起動します");
      let worker_props = Props::from_fn(FussyWorker::new);
      let child = ctx
        .spawn_child(&worker_props)
        .map_err(|error| ActorError::fatal(format!("子アクター生成に失敗: {:?}", error)))?;
      let actor_ref = child.actor_ref().clone();

      for index in 0..4 {
        ctx.log(LogLevel::Info, format!("トリガーを送信します (#{}).", index + 1));
        actor_ref
          .tell(AnyMessage::new(Trigger))
          .map_err(|error| ActorError::recoverable(format!("メッセージ送信に失敗: {:?}", error)))?;
        thread::sleep(Duration::from_millis(40));
      }

      ctx.stop_self().ok();
    }
    Ok(())
  }

  fn supervisor_strategy(&mut self, _ctx: &mut ActorContext<'_, '_>) -> SupervisorStrategy {
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, CoreDuration::from_secs(1), |error| match error {
      | ActorError::Recoverable(_) => SupervisorDirective::Restart,
      | ActorError::Fatal(_) => SupervisorDirective::Stop,
    })
  }
}

struct FussyWorker {
  crashes_remaining: i32,
}

impl FussyWorker {
  fn new() -> Self {
    Self { crashes_remaining: 2 }
  }
}

impl Actor for FussyWorker {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_, '_>) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "ワーカーが起動しました");
    Ok(())
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_, '_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if self.crashes_remaining >= 0 {
      ctx.log(LogLevel::Warn, format!("シミュレートされた障害を発生させます (残り {} 回)", self.crashes_remaining));
      self.crashes_remaining -= 1;
      Err(ActorError::recoverable("シミュレーション障害"))
    } else {
      ctx.log(LogLevel::Info, "正常に処理しました");
      ctx.stop_self().ok();
      Ok(())
    }
  }

  fn post_stop(&mut self, ctx: &mut ActorContext<'_, '_>) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "ワーカーを停止します");
    Ok(())
  }
}

fn main() {
  let props: Props = Props::from_fn(GuardianActor::new);
  let (tick_driver, _pulse_handle) = std_tick_driver_support::hardware_tick_driver_config();
  let system = ActorSystem::new(&props, tick_driver).expect("ガーディアンの起動に成功すること");

  let logger: EventStreamSubscriberShared =
    subscriber_handle(StdLoggerAdapter::new(LogLevel::Info, Box::new(StdoutLogger)));
  let _logger_subscription = system.subscribe_event_stream(&logger);

  let lifecycle: EventStreamSubscriberShared = subscriber_handle(LifecyclePrinter);
  let _lifecycle_subscription = system.subscribe_event_stream(&lifecycle);

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("ガーディアンに Start を送信できること");

  thread::sleep(Duration::from_millis(200));

  system.terminate().expect("システム停止要求が成功すること");
  let termination = system.when_terminated();
  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(20));
  }
}
