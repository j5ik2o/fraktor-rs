use std::{thread, time::Duration};

use fraktor_actor_rs::std::logging::TracingLoggerSubscriber;
use fraktor_actor_rs::{
  core::{
    error::ActorError,
    logging::LogLevel,
  },
  std::{
    actor_prim::{Actor, ActorContext, ActorRef},
    event_stream::EventStreamSubscriber,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    system::ActorSystem,
  },
};
use fraktor_utils_rs::core::sync::ArcShared;

struct Start;

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
  let subscriber = tracing_subscriber::FmtSubscriber::builder()
    .with_max_level(tracing::Level::DEBUG)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  let log_subscriber: ArcShared<dyn EventStreamSubscriber> =
    ArcShared::new(TracingLoggerSubscriber::new(LogLevel::Info));

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
