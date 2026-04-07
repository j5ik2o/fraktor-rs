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

use fraktor_actor_adaptor_std_rs::std::StdBlocker;
use fraktor_actor_core_rs::core::typed::{
  Behavior, TypedActorSystem, TypedProps,
  dsl::{Behaviors, TimerKey},
};
use fraktor_showcases_std::support;

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
    Behaviors::receive_message(move |_ctx, message: &Command| match message {
      | Command::Start => {
        // Part 1: 遅延実行（one-shot）
        println!("=== Part 1: One-shot timer ===");
        let once_key = TimerKey::new("delayed-hello");
        timers
          .lock()
          .start_single_timer(once_key, Command::DelayedHello, Duration::from_millis(50))
          .expect("schedule once");

        // Part 2: 定期実行
        println!("=== Part 2: Periodic timer ===");
        let periodic_key = TimerKey::new("periodic-tick");
        timers
          .lock()
          .start_timer_at_fixed_rate(periodic_key, Command::PeriodicTick, Duration::from_millis(40))
          .expect("schedule periodic");

        // Part 3: キャンセル
        println!("=== Part 3: Cancellation ===");
        let cancel_key = TimerKey::new("cancel-me");
        timers
          .lock()
          .start_single_timer(cancel_key.clone(), Command::ShouldNotArrive, Duration::from_millis(200))
          .expect("schedule to cancel");
        timers.lock().cancel(&cancel_key);
        println!("  timer 'cancel-me' をキャンセルしました");

        Ok(Behaviors::same())
      },
      | Command::DelayedHello => {
        println!("  [one-shot] DelayedHello を受信しました");
        Ok(Behaviors::same())
      },
      | Command::PeriodicTick => {
        let seq = tick_count.fetch_add(1, Ordering::Relaxed);
        println!("  [periodic] tick #{seq}");
        Ok(Behaviors::same())
      },
      | Command::ShouldNotArrive => {
        // キャンセルは best-effort のため、タイミングによっては到着しうる
        println!("  [cancel] キャンセル済みメッセージが到着しました（best-effort のため許容）");
        Ok(Behaviors::same())
      },
    })
  })
}

// --- エントリーポイント ---

#[allow(clippy::print_stdout)]
fn main() {
  use std::thread;

  let props = TypedProps::from_behavior_factory(timer_demo);
  let (tick_driver_config, _pulse_handle) = support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver_config).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(Command::Start);

  // タイマーが動作する時間を待つ
  thread::sleep(std::time::Duration::from_millis(300));

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
