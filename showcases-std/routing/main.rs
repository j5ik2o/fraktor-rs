//! Pool router with round-robin distribution.
//!
//! Demonstrates `Routers::pool` with round-robin strategy to distribute
//! work messages across multiple routee actors evenly.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example routing`

use std::time::{Duration, Instant};

use fraktor_actor_rs::{
  core::typed::{Routers, TypedActorSystem, TypedProps, actor::TypedActorRef},
  std::typed::Behaviors,
};
use fraktor_showcases_std::support;
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

// --- メッセージ定義 ---

#[derive(Clone)]
enum Command {
  Work(u32),
  Read { reply_to: TypedActorRef<Vec<(usize, u32)>> },
}

// --- Behavior 定義 ---

fn router_guardian(
  records: ArcShared<NoStdMutex<Vec<(usize, u32)>>>,
  next_routee_index: ArcShared<NoStdMutex<usize>>,
) -> TypedProps<Command> {
  TypedProps::from_behavior_factory(move || {
    let records = records.clone();
    let next_routee_index = next_routee_index.clone();
    let routee_factory = {
      let records = records.clone();
      let next_routee_index = next_routee_index.clone();
      move || {
        let routee_index = {
          let mut guard = next_routee_index.lock();
          let current = *guard;
          *guard += 1;
          current
        };
        let records = records.clone();
        Behaviors::receive_message(move |_ctx, message: &Command| match message {
          | Command::Work(value) => {
            records.lock().push((routee_index, *value));
            Ok(Behaviors::same())
          },
          | Command::Read { reply_to } => {
            let mut reply_to = reply_to.clone();
            reply_to.tell(records.lock().clone()).expect("reply");
            Ok(Behaviors::same())
          },
        })
      }
    };
    Routers::pool::<Command, _>(2, routee_factory).with_round_robin().build()
  })
}

// --- エントリーポイント ---

#[allow(clippy::print_stdout)]
fn main() {
  use std::thread;

  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let next_routee_index = ArcShared::new(NoStdMutex::new(0_usize));

  let props = router_guardian(records, next_routee_index);
  let (tick_driver_config, _pulse_handle) = support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver_config).expect("system");
  let mut router = system.user_guardian_ref();

  // 4つのワークメッセージを送信（2つのルーティーに分配される）
  for value in 0..4_u32 {
    router.tell(Command::Work(value)).expect("route work");
  }
  thread::sleep(Duration::from_millis(30));

  // 結果を読み取る
  let response = router.ask::<Vec<(usize, u32)>, _>(|reply_to| Command::Read { reply_to }).expect("ask");
  let mut future = response.future().clone();
  let deadline = Instant::now() + Duration::from_secs(1);
  while !future.is_ready() {
    assert!(Instant::now() < deadline, "ask timeout");
    thread::sleep(Duration::from_millis(10));
  }
  let records = future.try_take().expect("ready").expect("ok");
  println!("round robin records = {records:?}");

  system.terminate().expect("terminate");
}
