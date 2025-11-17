use std::{fmt::Write as _, thread, time::Duration};

use fraktor_actor_core_rs::{
  error::ActorError,
  logging::{LogEvent, LogLevel, LoggerSubscriber, LoggerWriter},
};
use fraktor_actor_std_rs::{
  actor_prim::{Actor, ActorContext, ActorRef},
  event_stream::EventStreamSubscriber,
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::ArcShared;

struct Start;

struct StdoutLogger;

impl LoggerWriter for StdoutLogger {
  fn write(&self, event: &LogEvent) {
    let mut origin = String::new();
    if let Some(pid) = event.origin() {
      let _ = write!(&mut origin, "{:?}", pid);
    } else {
      origin.push_str("system");
    }

    println!("[{:?}] {:?} origin={origin} message={}", event.timestamp(), event.level(), event.message());
  }
}

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      ctx.log(LogLevel::Debug, "debug は閾値未満なので無視される");
      ctx.log(LogLevel::Info, "INFO: ログ購読者がメッセージを受信しました");
      ctx.log(LogLevel::Warn, "WARN: イベントストリーム経由で通知されます");
      ctx.log(LogLevel::Error, "ERROR: 致命的なログも同じ経路で届きます");
      ctx.stop_self().ok();
    }
    Ok(())
  }
}

fn main() {
  let logger_writer: ArcShared<dyn LoggerWriter> = ArcShared::new(StdoutLogger);
  let log_subscriber: ArcShared<dyn EventStreamSubscriber> =
    ArcShared::new(LoggerSubscriber::new(LogLevel::Info, logger_writer));

  let props: Props = Props::from_fn(|| GuardianActor);
  let system = ActorSystem::new(&props).expect("actor system を初期化できること");

  let _subscription = system.subscribe_event_stream(&log_subscriber);

  let guardian: ActorRef = system.user_guardian_ref();
  guardian.tell(AnyMessage::new(Start)).expect("ガーディアンへ Start を送信できること");

  thread::sleep(Duration::from_millis(30));

  system.terminate().expect("システム停止要求が成功すること");
  let termination = system.when_terminated();
  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(10));
  }
}
