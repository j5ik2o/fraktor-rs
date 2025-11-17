#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::format;

use fraktor_actor_core_rs::core::{
  actor_prim::{Actor, ActorContext, ChildRef},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::ActorSystem,
};
use fraktor_utils_core_rs::runtime_toolbox::NoStdMutex, sync::ArcShared;

struct Start;
struct StopChild;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
  Idle,
  Watched,
  Done,
}

struct GuardianActor {
  child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>,
  phase:      ArcShared<NoStdMutex<Phase>>,
}

impl GuardianActor {
  fn new() -> Self {
    Self { child_slot: ArcShared::new(NoStdMutex::new(None)), phase: ArcShared::new(NoStdMutex::new(Phase::Idle)) }
  }

  fn spawn_worker(&self, ctx: &mut ActorContext<'_>) -> Result<ChildRef, ActorError> {
    let props = Props::from_fn(|| WorkerActor);
    ctx.spawn_child(&props).map_err(|error| ActorError::recoverable(format!("spawn failed: {:?}", error)))
  }

  fn stop_child(child_slot: &ArcShared<NoStdMutex<Option<ChildRef>>>) -> Result<(), ActorError> {
    if let Some(child) = child_slot.lock().as_ref() {
      child
        .tell(AnyMessage::new(StopChild))
        .map_err(|error| ActorError::recoverable(format!("stop request failed: {:?}", error)))
    } else {
      Ok(())
    }
  }
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let child = self.spawn_worker(ctx)?;
      ctx.watch(child.actor_ref()).map_err(|error| ActorError::recoverable(format!("watch failed: {:?}", error)))?;
      *self.phase.lock() = Phase::Watched;
      self.child_slot.lock().replace(child);
      #[cfg(not(target_os = "none"))]
      println!("[guardian] watch に登録しました");
      Self::stop_child(&self.child_slot)?;
    }
    Ok(())
  }

  fn on_terminated(
    &mut self,
    ctx: &mut ActorContext<'_>,
    pid: fraktor_actor_core_rs::core::actor_prim::Pid,
  ) -> Result<(), ActorError> {
    let mut phase = self.phase.lock();
    match *phase {
      | Phase::Watched => {
        #[cfg(not(target_os = "none"))]
        println!("[guardian] {:?} の停止通知を受信 -> watch の挙動", pid);
        *phase = Phase::Done;
        drop(phase);

        let child = self.spawn_worker(ctx)?;
        #[cfg(not(target_os = "none"))]
        println!("[guardian] 二体目は unwatch してから停止させます");
        ctx
          .unwatch(child.actor_ref())
          .map_err(|error| ActorError::recoverable(format!("unwatch failed: {:?}", error)))?;
        self.child_slot.lock().replace(child);
        Self::stop_child(&self.child_slot)?;
        ctx.stop_self().ok();
      },
      | Phase::Done => {
        #[cfg(not(target_os = "none"))]
        println!("[guardian] unwatch 済みでも通知が飛んできました (想定外)");
      },
      | Phase::Idle => {},
    }
    Ok(())
  }
}

struct WorkerActor;

impl Actor for WorkerActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<StopChild>().is_some() {
      ctx.stop_self().ok();
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::{thread, time::Duration};

  let props = Props::from_fn(GuardianActor::new);
  let system = ActorSystem::new(&props).expect("system");
  let termination = system.when_terminated();
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  thread::sleep(Duration::from_millis(200));
  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(20));
  }
}

#[cfg(target_os = "none")]
fn main() {}
