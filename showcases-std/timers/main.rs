//! Timer scheduling patterns.
//!
//! Demonstrates three timer operations using Pekko-style `Behaviors.withTimers`:
//!
//! 1. **One-shot** — send a message after a delay (`start_single_timer`).
//! 2. **Periodic** — send a message at a fixed rate (`start_timer_at_fixed_rate`).
//! 3. **Cancellation** — cancel a scheduled timer before it fires.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example timers`

use core::time::Duration;

use fraktor_actor_rs::core::typed::{Behavior, Behaviors, TimerKey, TypedActorSystem, TypedProps};
use fraktor_showcases_std::support;
use fraktor_utils_rs::core::sync::SharedAccess;

// --- メッセージ定義 ---

#[derive(Clone)]
enum Command {
  /// タイマーデモを開始する
  Start,
  /// 遅延実行メッセージ
  DelayedHello,
  /// 定期実行メッセージ
  PeriodicTick(u32),
  /// キャンセルされるべきメッセージ（到着しない）
  ShouldNotArrive,
}

// --- Behavior 定義 ---

fn timer_demo() -> Behavior<Command> {
  Behaviors::with_timers(|timers| {
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
          .start_timer_at_fixed_rate(periodic_key, Command::PeriodicTick(0), Duration::from_millis(40))
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
      | Command::PeriodicTick(seq) => {
        println!("  [periodic] tick #{seq}");
        Ok(Behaviors::same())
      },
      | Command::ShouldNotArrive => {
        // キャンセル済みのためここには到達しない
        println!("  [ERROR] キャンセルしたはずのメッセージが到着しました");
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

  system.user_guardian_ref().tell(Command::Start).expect("start");

  // タイマーが動作する時間を待つ
  thread::sleep(std::time::Duration::from_millis(300));

  system.terminate().expect("terminate");
  while !termination.with_read(|af| af.is_ready()) {
    thread::yield_now();
  }
}
