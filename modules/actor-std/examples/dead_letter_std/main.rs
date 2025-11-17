use core::num::NonZeroUsize;
use std::{thread, time::Duration};

use fraktor_actor_core_rs::core::{
  error::ActorError,
  logging::{LogEvent, LogLevel, LoggerSubscriber, LoggerWriter},
  mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  props::MailboxConfig,
};
use fraktor_actor_std_rs::{
  actor_prim::{Actor, ActorContext},
  dispatcher::dispatch_executor::TokioExecutor,
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::{ActorSystem, DispatcherConfig},
};
use fraktor_utils_core_rs::core::sync::ArcShared;
use tokio::runtime::Handle;

struct Start;

struct StdoutLogger;

impl LoggerWriter for StdoutLogger {
  fn write(&self, event: &LogEvent) {
    println!("[LOG {:?}] origin={:?} message={}", event.level(), event.origin(), event.message());
  }
}

struct DeadLetterPrinter;

impl EventStreamSubscriber for DeadLetterPrinter {
  fn on_event(&self, event: &EventStreamEvent) {
    if let EventStreamEvent::DeadLetter(entry) = event {
      println!(
        "[DEAD LETTER] reason={:?} recipient={:?} message_type={:?}",
        entry.reason(),
        entry.recipient(),
        entry.message().payload().type_id()
      );
    }
  }
}

struct GuardianActor {
  dispatcher: DispatcherConfig,
}

impl GuardianActor {
  fn new(dispatcher: DispatcherConfig) -> Self {
    Self { dispatcher }
  }
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      ctx.log(LogLevel::Info, "キャパシティ1のboundedキューでoverflowを発生させます");

      let mailbox_policy =
        MailboxPolicy::bounded(NonZeroUsize::new(1).unwrap(), MailboxOverflowStrategy::DropNewest, None);
      let mailbox_config = MailboxConfig::new(mailbox_policy);
      let overflow_props =
        Props::from_fn(|| OverflowActor).with_mailbox(mailbox_config).with_dispatcher(self.dispatcher.clone());

      let child = ctx
        .spawn_child(&overflow_props)
        .map_err(|error| ActorError::fatal(format!("子アクター生成に失敗: {:?}", error)))?;
      let actor_ref = child.actor_ref().clone();

      for index in 0..3 {
        let thread_id = format!("{:?}", thread::current().id());
        ctx.log(LogLevel::Info, format!("送信 #{} - tell()呼び出し前 [thread={}]", index + 1, thread_id));
        let result = actor_ref.tell(AnyMessage::new(match index {
          | 0 => "first",
          | 1 => "second",
          | _ => "third",
        }));
        ctx.log(LogLevel::Info, format!("送信 #{} - tell()呼び出し後 [thread={}]", index + 1, thread_id));
        match result {
          | Ok(()) => ctx.log(LogLevel::Info, format!("送信 #{} - 成功", index + 1)),
          | Err(error) => ctx.log(LogLevel::Warn, format!("送信 #{} - 失敗: {:?}", index + 1, error)),
        }
        thread::sleep(Duration::from_millis(40));
      }

      ctx.log(LogLevel::Info, "メッセージ送信完了");
      ctx.stop_self().ok();
    }
    Ok(())
  }
}

struct OverflowActor;

impl Actor for OverflowActor {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let thread_id = format!("{:?}", thread::current().id());
    ctx.log(LogLevel::Info, format!("OverflowActorが起動しました [thread={}]", thread_id));
    Ok(())
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    let thread_id = format!("{:?}", thread::current().id());
    if let Some(msg) = message.downcast_ref::<&str>() {
      ctx.log(LogLevel::Info, format!("[OverflowActor] received: {} [thread={}]", msg, thread_id));
    } else {
      ctx.log(LogLevel::Warn, format!("[OverflowActor] unknown message type [thread={}]", thread_id));
    }
    Ok(())
  }

  fn post_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "OverflowActorを停止します");
    Ok(())
  }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
  let handle = Handle::current();
  let dispatcher: DispatcherConfig = DispatcherConfig::from_executor(ArcShared::new(TokioExecutor::new(handle)));

  let props: Props = Props::from_fn({
    let dispatcher = dispatcher.clone();
    move || GuardianActor::new(dispatcher.clone())
  })
  .with_dispatcher(dispatcher.clone());

  let system = ActorSystem::new(&props).expect("actor system を初期化できること");

  let logger_writer: ArcShared<dyn LoggerWriter> = ArcShared::new(StdoutLogger);
  let logger: ArcShared<dyn EventStreamSubscriber> =
    ArcShared::new(LoggerSubscriber::new(LogLevel::Info, logger_writer));
  let _log_subscription = system.subscribe_event_stream(&logger);

  let printer: ArcShared<dyn EventStreamSubscriber> = ArcShared::new(DeadLetterPrinter);
  let _deadletter_subscription = system.subscribe_event_stream(&printer);

  println!("\n=== Starting overflow test ===\n");

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("ガーディアンに Start を送信できること");

  tokio::time::sleep(Duration::from_millis(200)).await;

  println!("\n=== Terminating system ===\n");
  system.terminate().expect("システム停止要求が成功すること");

  println!("\n=== Waiting for termination ===\n");
  let termination = system.when_terminated();
  termination.listener().await;

  println!("\n=== Example completed ===\n");
}
