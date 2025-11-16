use std::time::Duration;

use fraktor_actor_core_rs::{error::ActorError, scheduler::SchedulerCommand, system::ActorSystemBuilder};
use fraktor_actor_std_rs::{
  actor_prim::{Actor, ActorContext},
  dispatcher::dispatch_executor::TokioExecutor,
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::{ActorSystem, DispatcherConfig},
};
use fraktor_utils_core_rs::sync::ArcShared;
use tokio::runtime::Handle;
use fraktor_actor_std_rs::scheduler::tick::StdTickDriverConfig;

// アクターに送信されるスケジュール済みメッセージ
struct ScheduledMessage {
  text: String,
}

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      println!("[{:?}] Guardian starting scheduler example...", std::thread::current().id());

      // スケジューラを取得（システムから）
      // 注: 実際のシステムではスケジューラはシステムによって管理されるため、
      // この例ではスケジューラの使用方法を示すためのものです
      let target = ctx.self_ref();

      // 100msの遅延後にメッセージを送信するようスケジュール
      println!("[{:?}] Scheduling message with 100ms delay...", std::thread::current().id());

      let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
      let scheduler_arc = scheduler_context.scheduler();
      let mut scheduler = scheduler_arc.lock();

      let message = AnyMessage::new(ScheduledMessage { text: String::from("Hello from scheduler!") });
      let command = SchedulerCommand::SendMessage { receiver: target.clone(), message, dispatcher: None, sender: None };

      let _handle = scheduler
        .schedule_once(Duration::from_millis(100), command)
        .map_err(|_| ActorError::recoverable("failed to schedule"))?;

      println!("[{:?}] Scheduler ticks completed", std::thread::current().id());
    } else if let Some(msg) = message.downcast_ref::<ScheduledMessage>() {
      println!("[{:?}] Received scheduled message: {}", std::thread::current().id(), msg.text);
    }
    Ok(())
  }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
  let handle = Handle::current();
  let dispatcher: DispatcherConfig =
    DispatcherConfig::from_executor(ArcShared::new(TokioExecutor::new(handle.clone())));

  let props = Props::from_fn(|| GuardianActor).with_dispatcher(dispatcher);
  let core_system = ActorSystemBuilder::new(props.into_inner())
    .with_tick_driver(StdTickDriverConfig::tokio_quickstart())
    .build()
    .expect("system");

  let system = ActorSystem::from_core(core_system);

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  // スケジューラが動作する時間を与える
  tokio::time::sleep(Duration::from_millis(1000)).await;
  println!("[{:?}] Main thread finished waiting", std::thread::current().id());
}
