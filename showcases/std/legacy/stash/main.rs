//! Stash and unstash pattern.
//!
//! Demonstrates using `StashBuffer` to temporarily buffer messages while
//! an actor is in a "closed" state, then replaying them when it transitions
//! to an "open" state. The `unstash` call can also transform messages
//! before replaying.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example stash`

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_kernel_rs::actor::setup::ActorSystemConfig;
use fraktor_actor_core_typed_rs::{
  Behavior, TypedActorRef, TypedActorSystem, TypedProps,
  dsl::{Behaviors, StashBuffer},
};

// --- メッセージ定義 ---

#[derive(Clone)]
enum Command {
  /// 値をバッファに追加する（closed 状態では stash される）
  Buffer(i32),
  /// closed → open に遷移し、stash したメッセージを再生する
  Open,
  /// 現在の合計値を返す
  Read { reply_to: TypedActorRef<i32> },
}

// --- Behavior 定義 ---

/// 初期状態: StashBuffer を初期化して closed 状態に入る
fn buffering(total: i32) -> Behavior<Command> {
  Behaviors::with_stash(8, move |stash| closed(total, stash))
}

/// closed 状態: Buffer メッセージを stash し、Open で open に遷移する
fn closed(total: i32, stash: StashBuffer<Command>) -> Behavior<Command> {
  Behaviors::receive_message(move |ctx, message: &Command| match message {
    | Command::Buffer(_) => {
      stash.stash(ctx)?;
      Ok(Behaviors::same())
    },
    | Command::Open => {
      // stash したメッセージを最大2件再生し、値に +100 を加算して変換する
      let _ = stash.unstash(ctx, 2, |message| match message {
        | Command::Buffer(value) => Command::Buffer(value + 100),
        | other => other,
      })?;
      Ok(open(total))
    },
    | Command::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total);
      Ok(Behaviors::same())
    },
  })
}

/// open 状態: Buffer メッセージを即座に合計に加算する
fn open(total: i32) -> Behavior<Command> {
  Behaviors::receive_message(move |_ctx, message: &Command| match message {
    | Command::Buffer(value) => Ok(open(total + value)),
    | Command::Open => Ok(Behaviors::same()),
    | Command::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total);
      Ok(Behaviors::same())
    },
  })
}

// --- エントリーポイント ---

fn main() {
  use std::{
    thread,
    time::{Duration, Instant},
  };

  let props = TypedProps::from_behavior_factory(|| buffering(0)).with_stash_mailbox();
  let system =
    TypedActorSystem::create_from_props(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let mut actor = system.user_guardian_ref();

  // closed 状態で Buffer メッセージを送信（stash される）
  actor.tell(Command::Buffer(5));
  actor.tell(Command::Buffer(3));

  // Open で stash を再生して open 状態に遷移
  actor.tell(Command::Open);

  // unstash 後の合計を読み取る
  // 5 + 100 = 105, 3 + 100 = 103 → open(0) で 105 + 103 = 208
  thread::sleep(Duration::from_millis(50));
  let response = actor.ask::<i32, _>(|reply_to| Command::Read { reply_to });
  let mut future = response.future().clone();
  let deadline = Instant::now() + Duration::from_secs(1);
  while !future.is_ready() {
    assert!(Instant::now() < deadline, "ask timeout: Read did not return within 1s");
    thread::sleep(Duration::from_millis(10));
  }
  let value = future.try_take().expect("ready").expect("ok");
  println!("unstashed total = {value}");

  system.terminate().expect("terminate");
}
