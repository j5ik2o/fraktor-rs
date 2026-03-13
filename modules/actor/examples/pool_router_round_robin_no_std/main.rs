//! Demonstrates round-robin routing with a typed pool router.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use alloc::vec::Vec;

use fraktor_actor_rs::core::typed::{Behaviors, Routers, TypedActorSystem, TypedProps, actor::TypedActorRef};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

#[derive(Clone)]
enum Command {
  Work(u32),
  Read { reply_to: TypedActorRef<Vec<(usize, u32)>> },
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::{thread, time::Duration};

  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let next_routee_index = ArcShared::new(NoStdMutex::new(0_usize));

  let props = TypedProps::<Command>::from_behavior_factory({
    let records = records.clone();
    let next_routee_index = next_routee_index.clone();
    move || {
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
    }
  });

  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let mut router = system.user_guardian_ref();

  for value in 0..4_u32 {
    router.tell(Command::Work(value)).expect("route work");
  }
  thread::sleep(Duration::from_millis(30));

  let response = router.ask::<Vec<(usize, u32)>, _>(|reply_to| Command::Read { reply_to }).expect("ask");
  let mut future = response.future().clone();
  while !future.is_ready() {
    thread::sleep(Duration::from_millis(10));
  }
  let records = future.try_take().expect("ready").expect("ok");
  println!("round robin records = {:?}", records);

  system.terminate().expect("terminate");
}

#[cfg(target_os = "none")]
fn main() {
  // no_std ターゲットでは実行せず、ビルド専用のサンプルとして扱う。
}
