use std::{thread, time::Duration};

use cellactor_actor_core_rs::error::ActorError;
use cellactor_actor_std_rs::{
  actor_prim::{Actor, ActorContext, ChildRef},
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::ActorSystem,
};
use cellactor_utils_core_rs::sync::{ArcShared, NoStdMutex};

struct Start;
struct StopChild;
struct Crash;

struct Worker;

impl Actor for Worker {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Crash>().is_some() {
      println!("[worker] クラッシュ要求を受信しました");
      return Err(ActorError::recoverable("intentional crash"));
    }

    if message.downcast_ref::<StopChild>().is_some() {
      // 子アクターに停止を要求して DeathWatch の通知を発生させる。
      ctx.stop_self().ok();
    }
    Ok(())
  }
}

struct Guardian {
  last_child: ArcShared<NoStdMutex<Option<ChildRef>>>,
}

impl Guardian {
  fn new() -> Self {
    Self { last_child: ArcShared::new(NoStdMutex::new(None)) }
  }

  fn spawn_watched_child(&self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let props = Props::from_fn(|| Worker);
    let child =
      ctx.spawn_child_watched(&props).map_err(|error| ActorError::recoverable(format!("spawn failed: {:?}", error)))?;
    self.last_child.lock().replace(child);
    Ok(())
  }
}

impl Actor for Guardian {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    println!("[guardian] 子アクターを監視付きで生成します");
    self.spawn_watched_child(ctx)
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some()
      && let Some(child) = self.last_child.lock().as_ref()
    {
      println!("[guardian] 子アクターにクラッシュを指示します");
      child.tell(AnyMessage::new(Crash)).map_err(|_| ActorError::recoverable("tell failed"))?;
      println!("[guardian] 再起動後に停止指示を送ります");
      child.tell(AnyMessage::new(StopChild)).map_err(|_| ActorError::recoverable("tell failed"))?;
    }
    Ok(())
  }

  fn on_terminated(
    &mut self,
    ctx: &mut ActorContext<'_>,
    pid: cellactor_actor_core_rs::actor_prim::Pid,
  ) -> Result<(), ActorError> {
    println!("[guardian] 監視対象 {:?} の停止を検知", pid);
    println!("[guardian] DeathWatch トリガー後に子アクターを再生成します");
    self.spawn_watched_child(ctx)?;
    Ok(())
  }
}

fn main() {
  let props = Props::from_fn(Guardian::new);
  let system = ActorSystem::new(&props).expect("build actor system");

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("send start");

  thread::sleep(Duration::from_millis(200));
  system.terminate().expect("terminate");
  let termination = system.when_terminated();
  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(20));
  }
}
