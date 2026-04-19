//! Child actor lifecycle management.
//!
//! Demonstrates spawning a child actor, watching it for termination,
//! receiving the `Terminated` signal, and applying a `SupervisorStrategy`
//! to restart children on recoverable failures.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example child_lifecycle`

use core::{
  sync::atomic::{AtomicU32, Ordering},
  time::Duration,
};
use std::sync::Arc;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::actor::{
    error::ActorError,
    setup::ActorSystemConfig,
    supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  },
  typed::{Behavior, TypedActorSystem, TypedProps, dsl::Behaviors, message_and_signals::BehaviorSignal},
};

// --- メッセージ定義 ---

#[derive(Clone, Copy)]
enum ParentCommand {
  Start,
}

#[derive(Clone, Copy)]
enum ChildCommand {
  Work,
  Crash,
}

// --- 子アクター: 2回クラッシュした後に正常処理する ---
// crashes_remaining を Arc<AtomicU32> で共有し、リスタートを跨いでカウントを維持する

fn fussy_worker(crashes_remaining: Arc<AtomicU32>) -> Behavior<ChildCommand> {
  Behaviors::receive_message(move |_ctx, message: &ChildCommand| match message {
    | ChildCommand::Work => {
      println!("  [child] 正常に処理しました");
      Ok(Behaviors::same())
    },
    | ChildCommand::Crash => {
      let remaining = crashes_remaining.load(Ordering::Acquire);
      if remaining > 0 {
        crashes_remaining.fetch_sub(1, Ordering::Release);
        println!("  [child] クラッシュします (残り {} 回)", remaining - 1);
        Err(ActorError::recoverable("simulated crash"))
      } else {
        println!("  [child] クラッシュ回数を超過、正常処理に切り替え");
        Ok(Behaviors::same())
      }
    },
  })
}

// --- 親アクター: 子を spawn_child_watched し、Terminated シグナルを受信する ---

fn parent() -> Behavior<ParentCommand> {
  // SupervisorStrategy: 子の recoverable エラーを最大3回まで再起動
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(1), |error| match error {
      | ActorError::Recoverable(_) => SupervisorDirective::Restart,
      | ActorError::Fatal(_) => SupervisorDirective::Stop,
      | ActorError::Escalate(_) => SupervisorDirective::Escalate,
    });

  let inner = Behaviors::setup(move |ctx| {
    // 子アクターを監視付きで生成
    let crashes = Arc::new(AtomicU32::new(2));
    let child = ctx
      .spawn_child_watched(&TypedProps::from_behavior_factory(move || fussy_worker(crashes.clone())))
      .expect("spawn child");
    let child_ref = child.actor_ref();

    Behaviors::receive_message(move |_ctx, message: &ParentCommand| match message {
      | ParentCommand::Start => {
        println!("[parent] 子アクターにクラッシュを指示します");
        let mut child = child_ref.clone();
        child.tell(ChildCommand::Crash);
        child.tell(ChildCommand::Crash);
        child.tell(ChildCommand::Work);
        Ok(Behaviors::same())
      },
    })
    .receive_signal(|_ctx, signal| {
      match signal {
        | BehaviorSignal::Terminated(terminated) => {
          println!("[parent] 子アクター {:?} の停止を検知 (Terminated)", terminated.pid());
        },
        | BehaviorSignal::ChildFailed(child_failed) => {
          println!("[parent] 子アクター {:?} が失敗: {:?}", child_failed.pid(), child_failed.error());
        },
        | BehaviorSignal::PreRestart => {
          println!("[parent] PreRestart シグナル受信");
        },
        | _ => {},
      }
      Ok(Behaviors::same())
    })
  });

  Behaviors::supervise(inner).on_failure(strategy)
}

// --- エントリーポイント ---

#[allow(clippy::print_stdout)]
fn main() {
  use std::thread;

  let props = TypedProps::from_behavior_factory(parent);
  let system =
    TypedActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();

  // 親アクターにメッセージを送信して子ライフサイクルのデモを開始
  system.user_guardian_ref().tell(ParentCommand::Start);

  // supervision と再起動が行われる時間を待つ
  thread::sleep(std::time::Duration::from_millis(300));

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
