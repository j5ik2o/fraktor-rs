#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::string::String;
use core::time::Duration;
#[cfg(not(target_os = "none"))]
use std::{process, thread, time::Duration as StdDuration};

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContext},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  scheduler::SchedulerCommand,
  system::ActorSystemBuilder,
};

#[cfg(not(target_os = "none"))]
#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;
#[cfg(not(target_os = "none"))]

// アクターに送信されるスケジュール済みメッセージ
struct ScheduledMessage {
  text: String,
}

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageViewGeneric<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Guardian starting scheduler example...", std::thread::current().id());

      // スケジューラを取得（システムから）
      // 注: 実際のシステムではスケジューラはシステムによって管理されるため、
      // この例ではスケジューラの使用方法を示すためのものです
      let target = ctx.self_ref();

      // 100msの遅延後にメッセージを送信するようスケジュール
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Scheduling message with 100ms delay...", std::thread::current().id());

      let scheduler_context = ctx.system().scheduler_context().expect("scheduler context");
      let scheduler_arc = scheduler_context.scheduler();
      let mut scheduler = scheduler_arc.lock();

      let message = AnyMessage::new(ScheduledMessage { text: String::from("Hello from scheduler!") });
      let command = SchedulerCommand::SendMessage { receiver: target.clone(), message, dispatcher: None, sender: None };

      let _handle = scheduler
        .schedule_once(Duration::from_millis(100), command)
        .map_err(|_| ActorError::recoverable("failed to schedule"))?;

      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Scheduler ticks completed", std::thread::current().id());
    } else if let Some(msg) = message.downcast_ref::<ScheduledMessage>() {
      #[cfg(not(target_os = "none"))]
      println!("[{:?}] Received scheduled message: {}", std::thread::current().id(), msg.text);
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  let props = Props::from_fn(|| GuardianActor);
  let system = ActorSystemBuilder::new(props)
    .with_tick_driver(no_std_tick_driver_support::hardware_tick_driver_config())
    .build()
    .expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  // スケジューラが動作する時間を与える
  thread::sleep(StdDuration::from_millis(200));

  process::exit(0);
}

#[cfg(target_os = "none")]
fn main() {}
