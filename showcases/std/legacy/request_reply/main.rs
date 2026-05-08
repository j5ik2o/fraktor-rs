//! Request-reply pattern using `ask`.
//!
//! A requester actor sends an ask-style request to a responder actor.
//! The response is piped back as a typed message, demonstrating Pekko's
//! `pipeToSelf(target.ask(...))` pattern in fraktor-rs.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example request_reply`

use core::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};
use std::sync::Arc;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::{
    actor::{error::ActorError, setup::ActorSystemConfig},
    event::logging::LogLevel,
  },
  typed::{Behavior, TypedActorRef, TypedActorSystem, TypedProps, dsl::Behaviors},
};
use fraktor_showcases_std::subscribe_typed_tracing_logger;

// --- メッセージ定義 ---

#[derive(Clone)]
enum ResponderMsg {
  GetValue { reply_to: TypedActorRef<u32> },
}

#[derive(Clone)]
enum RequesterMsg {
  Start,
  GotResponse(u32),
  GotFailure,
}

// --- Behavior 定義 ---

fn responder() -> Behavior<ResponderMsg> {
  Behaviors::receive_message(|_ctx, msg: &ResponderMsg| match msg {
    | ResponderMsg::GetValue { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(42);
      Ok(Behaviors::same())
    },
  })
}

fn requester(done: Arc<AtomicBool>) -> Behavior<RequesterMsg> {
  Behaviors::setup(move |ctx| {
    // 子アクターとして responder を生成
    let responder = ctx.spawn_child(&TypedProps::from_behavior_factory(responder)).expect("spawn responder");
    let done = done.clone();

    Behaviors::receive_message(move |ctx, msg: &RequesterMsg| match msg {
      | RequesterMsg::Start => {
        let mut target = responder.actor_ref();
        ctx
          .ask(
            &mut target,
            |reply_to| ResponderMsg::GetValue { reply_to },
            |result| match result {
              | Ok(value) => RequesterMsg::GotResponse(value),
              | Err(_) => RequesterMsg::GotFailure,
            },
            Duration::from_secs(5),
          )
          .map_err(|e| ActorError::recoverable(format!("ask failed: {e:?}")))?;
        Ok(Behaviors::same())
      },
      | RequesterMsg::GotResponse(value) => {
        ctx.system().emit_log(LogLevel::Info, format!("received response: {value}"), Some(ctx.pid()), None);
        done.store(true, Ordering::Release);
        Ok(Behaviors::same())
      },
      | RequesterMsg::GotFailure => {
        ctx.system().emit_log(LogLevel::Warn, "ask failed (timeout or error)", Some(ctx.pid()), None);
        done.store(true, Ordering::Release);
        Ok(Behaviors::same())
      },
    })
  })
}

// --- エントリーポイント ---

fn main() {
  use std::{thread, time::Instant};

  let done = Arc::new(AtomicBool::new(false));
  let done_clone = done.clone();
  let system = TypedActorSystem::create_from_behavior_factory(
    move || requester(done_clone.clone()),
    ActorSystemConfig::new(StdTickDriver::default()),
  )
  .expect("system");
  let _log_subscription = subscribe_typed_tracing_logger(&system);
  let termination = system.when_terminated();

  // ask リクエストを開始
  let mut guardian = system.user_guardian_ref();
  guardian.try_tell(RequesterMsg::Start).expect("enqueue RequesterMsg::Start");

  // ask の完了をフラグで待機
  let deadline = Instant::now() + Duration::from_secs(5);
  while !done.load(Ordering::Acquire) {
    if Instant::now() >= deadline {
      panic!("timed out waiting for request_reply done flag after {:?}", Duration::from_secs(5));
    }
    thread::sleep(Duration::from_millis(1));
  }
  println!("request_reply completed ask-style request");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
