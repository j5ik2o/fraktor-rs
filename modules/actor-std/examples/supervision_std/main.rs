use core::time::Duration as CoreDuration;
use std::{thread, time::Duration};

use cellactor_actor_core_rs::{
  error::ActorError,
  logging::{LogEvent, LogLevel, LoggerSubscriber, LoggerWriter},
  supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
};
use cellactor_actor_std_rs::{
  actor_prim::{Actor, ActorContext},
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::ActorSystem,
};
use cellactor_utils_core_rs::sync::ArcShared;

struct Start;
struct Trigger;

struct StdoutLogger;

impl LoggerWriter for StdoutLogger {
  fn write(&self, event: &LogEvent) {
    println!("[LOG {:?}] origin={:?} message={}", event.level(), event.origin(), event.message());
  }
}

struct LifecyclePrinter;

impl EventStreamSubscriber for LifecyclePrinter {
  fn on_event(&self, event: &EventStreamEvent) {
    match event {
      | EventStreamEvent::Lifecycle(lifecycle) => {
        println!("[LIFECYCLE] pid={:?} stage={:?}", lifecycle.pid(), lifecycle.stage());
      },
      | EventStreamEvent::DeadLetter(entry) => {
        println!("[DEAD LETTER] reason={:?} recipient={:?}", entry.reason(), entry.recipient());
      },
      | EventStreamEvent::Log(_) | EventStreamEvent::Mailbox(_) | EventStreamEvent::UnhandledMessage(_) => {},
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
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
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

  fn supervisor_strategy(&mut self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategy {
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
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "ワーカーが起動しました");
    Ok(())
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
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

  fn post_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "ワーカーを停止します");
    Ok(())
  }
}

fn main() {
  let props: Props = Props::from_fn(GuardianActor::new);
  let system = ActorSystem::new(&props).expect("ガーディアンの起動に成功すること");

  let logger_writer: ArcShared<dyn LoggerWriter> = ArcShared::new(StdoutLogger);
  let logger: ArcShared<dyn EventStreamSubscriber> =
    ArcShared::new(LoggerSubscriber::new(LogLevel::Info, logger_writer));
  let _logger_subscription = system.subscribe_event_stream(&logger);

  let lifecycle: ArcShared<dyn EventStreamSubscriber> = ArcShared::new(LifecyclePrinter);
  let _lifecycle_subscription = system.subscribe_event_stream(&lifecycle);

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("ガーディアンに Start を送信できること");

  thread::sleep(Duration::from_millis(200));

  system.terminate().expect("システム停止要求が成功すること");
  let termination = system.when_terminated();
  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(20));
  }
}
