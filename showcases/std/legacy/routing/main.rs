//! Pool router with round-robin distribution.
//!
//! Demonstrates `Routers::pool` with round-robin strategy to distribute
//! work messages across multiple routee actors evenly.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example routing`

use std::time::{Duration, Instant};

use fraktor_actor_adaptor_std_rs::tick_driver::StdTickDriver;
use fraktor_actor_core_kernel_rs::actor::setup::ActorSystemConfig;
use fraktor_actor_core_typed_rs::{
  TypedActorRef, TypedActorSystem, TypedProps,
  dsl::{Behaviors, routing::Routers},
};
use fraktor_utils_core_rs::sync::{SharedLock, SpinSyncMutex};

// --- メッセージ定義 ---

#[derive(Clone)]
enum Command {
  Work(u32),
  Read { reply_to: TypedActorRef<Vec<(usize, u32)>> },
}

// --- Behavior 定義 ---

fn router_guardian(
  records: SharedLock<Vec<(usize, u32)>>,
  next_routee_index: SharedLock<usize>,
) -> TypedProps<Command> {
  TypedProps::from_behavior_factory(move || {
    let records = records.clone();
    let next_routee_index = next_routee_index.clone();
    let routee_factory = {
      let records = records.clone();
      let next_routee_index = next_routee_index.clone();
      move || {
        let routee_index = next_routee_index.with_lock(|next_routee_index| {
          let current = *next_routee_index;
          *next_routee_index += 1;
          current
        });
        let records = records.clone();
        Behaviors::receive_message(move |_ctx, message: &Command| match message {
          | Command::Work(value) => {
            records.with_lock(|records| records.push((routee_index, *value)));
            Ok(Behaviors::same())
          },
          | Command::Read { reply_to } => {
            let mut reply_to = reply_to.clone();
            reply_to.tell(records.with_lock(|records| records.clone()));
            Ok(Behaviors::same())
          },
        })
      }
    };
    Routers::pool::<Command, _>(2, routee_factory).with_round_robin()
  })
}

// --- エントリーポイント ---

fn main() {
  use std::thread;

  let records = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let next_routee_index = SharedLock::new_with_driver::<SpinSyncMutex<_>>(0_usize);

  let props = router_guardian(records, next_routee_index);
  let system =
    TypedActorSystem::create_from_props(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let mut router = system.user_guardian_ref();

  // 4つのワークメッセージを送信（2つのルーティーに分配される）
  for value in 0..4_u32 {
    router.tell(Command::Work(value));
  }
  thread::sleep(Duration::from_millis(30));

  // 結果を読み取り、全ワークが反映されるまで短時間ポーリングする
  let deadline = Instant::now() + Duration::from_secs(3);
  loop {
    let response = router.ask::<Vec<(usize, u32)>, _>(|reply_to| Command::Read { reply_to });
    let mut future = response.future().clone();
    let ask_deadline = Instant::now() + Duration::from_secs(1);
    while !future.is_ready() {
      assert!(Instant::now() < ask_deadline, "ask timeout");
      thread::sleep(Duration::from_millis(10));
    }
    let records = future.try_take().expect("ready").expect("ok");
    let observed_count = records.len();
    if observed_count == 4 {
      println!("routing distributed records: {records:?}");
      break;
    }
    assert!(Instant::now() < deadline, "all work items should be routed within 3 seconds, observed {}", observed_count);
    thread::sleep(Duration::from_millis(20));
  }

  system.terminate().expect("terminate");
}
