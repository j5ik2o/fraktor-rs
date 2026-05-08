//! Timer scheduling patterns.
//!
//! Demonstrates three timer operations using Pekko-style `Behaviors.withTimers`:
//!
//! 1. **One-shot** — send a message after a delay (`start_single_timer`).
//! 2. **Periodic** — send a message at a fixed rate (`start_timer_at_fixed_rate`).
//! 3. **Cancellation** — cancel a scheduled timer before it fires.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example timers`

use core::{
  sync::atomic::{AtomicU32, Ordering},
  time::Duration,
};
use std::sync::Arc;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::{actor::setup::ActorSystemConfig, event::logging::LogLevel},
  typed::{
    Behavior, TypedActorSystem, TypedProps,
    dsl::{Behaviors, TimerKey},
  },
};
use fraktor_showcases_std::subscribe_typed_tracing_logger;

// --- メッセージ定義 ---

#[derive(Clone)]
enum Command {
  /// タイマーデモを開始する
  Start,
  /// 遅延実行メッセージ
  DelayedHello,
  /// 定期実行メッセージ
  PeriodicTick,
  /// キャンセルされるべきメッセージ（到着しない）
  ShouldNotArrive,
}

// --- Behavior 定義 ---

fn timer_demo() -> Behavior<Command> {
  Behaviors::with_timers(|timers| {
    let tick_count = Arc::new(AtomicU32::new(0));
    Behaviors::receive_message(move |ctx, message: &Command| match message {
      | Command::Start => {
        // Part 1: 遅延実行（one-shot）
        ctx.system().emit_log(LogLevel::Info, "starting one-shot timer", Some(ctx.pid()), None);
        let once_key = TimerKey::new("delayed-hello");
        timers
          .with_lock(|timers| timers.start_single_timer(once_key, Command::DelayedHello, Duration::from_millis(50)))
          .expect("schedule once");

        // Part 2: 定期実行
        ctx.system().emit_log(LogLevel::Info, "starting periodic timer", Some(ctx.pid()), None);
        let periodic_key = TimerKey::new("periodic-tick");
        timers
          .with_lock(|timers| {
            timers.start_timer_at_fixed_rate(periodic_key, Command::PeriodicTick, Duration::from_millis(40))
          })
          .expect("schedule periodic");

        // Part 3: キャンセル
        ctx.system().emit_log(LogLevel::Info, "starting cancellable timer", Some(ctx.pid()), None);
        let cancel_key = TimerKey::new("cancel-me");
        timers
          .with_lock(|timers| {
            timers.start_single_timer(cancel_key.clone(), Command::ShouldNotArrive, Duration::from_millis(200))
          })
          .expect("schedule to cancel");
        timers.with_lock(|timers| timers.cancel(&cancel_key));
        ctx.system().emit_log(LogLevel::Info, "cancelled timer 'cancel-me'", Some(ctx.pid()), None);

        Ok(Behaviors::same())
      },
      | Command::DelayedHello => {
        ctx.system().emit_log(LogLevel::Info, "received DelayedHello from one-shot timer", Some(ctx.pid()), None);
        Ok(Behaviors::same())
      },
      | Command::PeriodicTick => {
        let seq = tick_count.fetch_add(1, Ordering::Relaxed);
        ctx.system().emit_log(LogLevel::Info, format!("received periodic tick #{seq}"), Some(ctx.pid()), None);
        Ok(Behaviors::same())
      },
      | Command::ShouldNotArrive => {
        // キャンセルは best-effort のため、タイミングによっては到着しうる
        ctx.system().emit_log(LogLevel::Warn, "received cancelled timer message", Some(ctx.pid()), None);
        Ok(Behaviors::same())
      },
    })
  })
}

// --- エントリーポイント ---

fn main() {
  use std::thread;

  let props = TypedProps::from_behavior_factory(timer_demo);
  let system =
    TypedActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let _log_subscription = subscribe_typed_tracing_logger(&system);
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(Command::Start);

  // タイマーが動作する時間を待つ
  thread::sleep(std::time::Duration::from_millis(300));
  println!("timers completed one-shot, periodic, and cancellation flow");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
