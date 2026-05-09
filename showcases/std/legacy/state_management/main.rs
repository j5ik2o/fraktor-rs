//! State management with behavior transitions.
//!
//! Demonstrates two patterns for managing actor state:
//!
//! 1. **Functional counter** — each `Add` returns a new behavior capturing the updated total
//!    (immutable state transitions).
//! 2. **Turnstile state machine** — `locked` and `unlocked` behaviors replace each other on
//!    `InsertCoin` / `PassThrough` events.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example state_management`

use core::time::Duration;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::{actor::setup::ActorSystemConfig, event::logging::LogLevel};
use fraktor_actor_typed_rs::{Behavior, TypedActorRef, TypedActorSystem, dsl::Behaviors};
use fraktor_showcases_std::subscribe_typed_tracing_logger;

// =============================================================================
// パート 1: カウンターアクター（イミュータブルな状態遷移）
// =============================================================================

#[derive(Clone)]
enum CounterCommand {
  Add(i32),
  Read { reply_to: TypedActorRef<i32> },
}

fn counter(total: i32) -> Behavior<CounterCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | CounterCommand::Add(delta) => {
      // 新しい Behavior を返すことで状態を遷移させる
      Ok(counter(total + delta))
    },
    | CounterCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total);
      Ok(Behaviors::same())
    },
  })
}

// =============================================================================
// パート 2: 改札ゲート（ステートマシン）
// =============================================================================

#[derive(Clone)]
enum GateCommand {
  InsertCoin,
  PassThrough,
  ReadPassCount { reply_to: TypedActorRef<u32> },
  Shutdown,
}

fn locked(pass_count: u32) -> Behavior<GateCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | GateCommand::InsertCoin => {
      ctx.system().emit_log(LogLevel::Info, "locked -> unlocked", Some(ctx.pid()), None);
      Ok(unlocked(pass_count))
    },
    | GateCommand::PassThrough => {
      ctx.system().emit_log(LogLevel::Info, "locked: PassThrough ignored", Some(ctx.pid()), None);
      Ok(Behaviors::ignore())
    },
    | GateCommand::ReadPassCount { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(pass_count);
      Ok(Behaviors::same())
    },
    | GateCommand::Shutdown => Ok(Behaviors::stopped()),
  })
}

fn unlocked(pass_count: u32) -> Behavior<GateCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | GateCommand::PassThrough => {
      let next_total = pass_count + 1;
      ctx.system().emit_log(LogLevel::Info, format!("unlocked -> locked (total {next_total})"), Some(ctx.pid()), None);
      Ok(locked(next_total))
    },
    | GateCommand::InsertCoin => {
      ctx.system().emit_log(LogLevel::Info, "unlocked: extra InsertCoin ignored", Some(ctx.pid()), None);
      Ok(Behaviors::ignore())
    },
    | GateCommand::ReadPassCount { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(pass_count);
      Ok(Behaviors::same())
    },
    | GateCommand::Shutdown => Ok(Behaviors::stopped()),
  })
}

// --- エントリーポイント ---

fn main() {
  println!("=== Part 1: Counter ===");
  run_counter();

  println!();
  println!("=== Part 2: Turnstile Gate ===");
  run_gate();
}

fn run_counter() {
  use std::{thread, time::Instant};

  let system =
    TypedActorSystem::create_from_behavior_factory(|| counter(0), ActorSystemConfig::new(StdTickDriver::default()))
      .expect("system");
  let mut counter_ref = system.user_guardian_ref();
  let termination = system.when_terminated();

  counter_ref.tell(CounterCommand::Add(4));
  counter_ref.tell(CounterCommand::Add(6));

  let response = counter_ref.ask::<i32, _>(|reply_to| CounterCommand::Read { reply_to });
  let mut future = response.future().clone();
  let started_at = Instant::now();
  let timeout = Duration::from_secs(5);
  while !future.is_ready() {
    if started_at.elapsed() >= timeout {
      panic!("timed out waiting for counter Read future.is_ready() after {:?}", timeout);
    }
    thread::sleep(Duration::from_millis(1));
  }
  if let Some(result) = future.try_take() {
    match result {
      | Ok(value) => println!("counter result: {value}"),
      | Err(error) => println!("counter error: {error}"),
    }
  }

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn run_gate() {
  use std::{thread, time::Instant};

  let system =
    TypedActorSystem::create_from_behavior_factory(|| locked(0), ActorSystemConfig::new(StdTickDriver::default()))
      .expect("system");
  let _log_subscription = subscribe_typed_tracing_logger(&system);
  let mut gate = system.user_guardian_ref();
  let termination = system.when_terminated();

  // コインなしで通過を試みる（拒否される）
  gate.tell(GateCommand::PassThrough);
  // コインを投入してゲートを開く
  gate.tell(GateCommand::InsertCoin);
  // 余分なコイン（無視される）
  gate.tell(GateCommand::InsertCoin);
  // 通過（ゲートが閉まる）
  gate.tell(GateCommand::PassThrough);

  let response = gate.ask::<u32, _>(|reply_to| GateCommand::ReadPassCount { reply_to });
  let mut future = response.future().clone();
  let started_at = Instant::now();
  let timeout = Duration::from_secs(5);
  while !future.is_ready() {
    if started_at.elapsed() >= timeout {
      panic!("timed out waiting for gate ReadPassCount future.is_ready() after {:?}", timeout);
    }
    thread::sleep(Duration::from_millis(1));
  }
  if let Some(result) = future.try_take() {
    match result {
      | Ok(total) => println!("gate allowed {total} people"),
      | Err(error) => println!("gate count error: {error}"),
    }
  }

  gate.tell(GateCommand::Shutdown);

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
