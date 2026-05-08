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
  kernel::{
    actor::{
      error::ActorError,
      setup::ActorSystemConfig,
      supervision::{RestartLimit, SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
    },
    event::logging::LogLevel,
  },
  typed::{Behavior, TypedActorSystem, TypedProps, dsl::Behaviors, message_and_signals::BehaviorSignal},
};
use fraktor_showcases_std::subscribe_typed_tracing_logger;

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
  Behaviors::receive_message(move |ctx, message: &ChildCommand| match message {
    | ChildCommand::Work => {
      ctx.system().emit_log(LogLevel::Info, "[child] processed work", Some(ctx.pid()), None);
      Ok(Behaviors::same())
    },
    | ChildCommand::Crash => {
      let remaining = crashes_remaining.load(Ordering::Acquire);
      if remaining > 0 {
        crashes_remaining.fetch_sub(1, Ordering::Release);
        ctx.system().emit_log(
          LogLevel::Warn,
          format!("[child] crashing, remaining restarts: {}", remaining - 1),
          Some(ctx.pid()),
          None,
        );
        Err(ActorError::recoverable("simulated crash"))
      } else {
        ctx.system().emit_log(LogLevel::Info, "[child] switching back to normal work", Some(ctx.pid()), None);
        Ok(Behaviors::same())
      }
    },
  })
}

// --- 親アクター: 子を spawn_child_watched し、Terminated シグナルを受信する ---

fn parent() -> Behavior<ParentCommand> {
  // SupervisorStrategy: 子の recoverable エラーを最大3回まで再起動
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(3),
    Duration::from_secs(1),
    |error| match error {
      | ActorError::Recoverable(_) => SupervisorDirective::Restart,
      | ActorError::Fatal(_) => SupervisorDirective::Stop,
      | ActorError::Escalate(_) => SupervisorDirective::Escalate,
    },
  );

  let inner = Behaviors::setup(move |ctx| {
    // 子アクターを監視付きで生成
    let crashes = Arc::new(AtomicU32::new(2));
    let child = ctx
      .spawn_child_watched(&TypedProps::from_behavior_factory(move || fussy_worker(crashes.clone())))
      .expect("spawn child");
    let child_ref = child.actor_ref();

    Behaviors::receive_message(move |ctx, message: &ParentCommand| match message {
      | ParentCommand::Start => {
        ctx.system().emit_log(LogLevel::Info, "[parent] sending crash commands to child", Some(ctx.pid()), None);
        let mut child = child_ref.clone();
        child.tell(ChildCommand::Crash);
        child.tell(ChildCommand::Crash);
        child.tell(ChildCommand::Work);
        Ok(Behaviors::same())
      },
    })
    .receive_signal(|ctx, signal| {
      match signal {
        | BehaviorSignal::Terminated(terminated) => {
          ctx.system().emit_log(
            LogLevel::Info,
            format!("[parent] observed child termination: {:?}", terminated.pid()),
            Some(ctx.pid()),
            None,
          );
        },
        | BehaviorSignal::ChildFailed(child_failed) => {
          ctx.system().emit_log(
            LogLevel::Warn,
            format!("[parent] child failed: {:?}: {:?}", child_failed.pid(), child_failed.error()),
            Some(ctx.pid()),
            None,
          );
        },
        | BehaviorSignal::PreRestart => {
          ctx.system().emit_log(LogLevel::Info, "[parent] received PreRestart", Some(ctx.pid()), None);
        },
        | _ => {},
      }
      Ok(Behaviors::same())
    })
  });

  Behaviors::supervise(inner).on_failure(strategy)
}

// --- エントリーポイント ---

fn main() {
  use std::thread;

  let props = TypedProps::from_behavior_factory(parent);
  let system =
    TypedActorSystem::create_from_props(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let _log_subscription = subscribe_typed_tracing_logger(&system);
  let termination = system.when_terminated();

  // 親アクターにメッセージを送信して子ライフサイクルのデモを開始
  system.user_guardian_ref().tell(ParentCommand::Start);

  // supervision と再起動が行われる時間を待つ
  thread::sleep(std::time::Duration::from_millis(300));
  println!("child_lifecycle completed supervised child restart flow");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
