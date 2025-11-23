use fraktor_actor_rs::{
  core::error::ActorError,
  std::{
    actor_prim::{Actor, ActorContext},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
};

use crate::{printer::PrinterActor, start_message::Start};

pub struct GuardianActor;

impl GuardianActor {
  fn spawn_named_child(&self, ctx: &ActorContext<'_, '_>, name: &str) {
    let props = Props::from_fn(|| PrinterActor).with_name(name.to_string());
    match ctx.spawn_child(&props) {
      | Ok(child) => {
        println!("[guardian] 名前付き子アクターを生成しました name={} pid={}", name, child.pid());
        let payload = format!("{} として起動しました", name);
        if let Err(error) = child.tell(AnyMessage::new(payload)) {
          println!("[guardian] 子アクターへの送信に失敗 name={} error={:?}", name, error);
        }
      },
      | Err(error) => {
        println!("[guardian] 子アクターの生成に失敗 name={} error={:?}", name, error);
      },
    }
  }

  fn spawn_anonymous_child(&self, ctx: &ActorContext<'_, '_>) {
    let props = Props::from_fn(|| PrinterActor);
    match ctx.spawn_child(&props) {
      | Ok(child) => {
        println!("[guardian] 匿名子アクターを生成しました pid={}", child.pid());
        let payload = "匿名アクターへの挨拶".to_string();
        if let Err(error) = child.tell(AnyMessage::new(payload)) {
          println!("[guardian] 匿名アクターへの送信に失敗 error={:?}", error);
        }
      },
      | Err(error) => {
        println!("[guardian] 匿名アクターの生成に失敗 error={:?}", error);
      },
    }
  }
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      self.spawn_named_child(ctx, "worker-main");
      self.spawn_named_child(ctx, "worker-main");
      self.spawn_anonymous_child(ctx);
      ctx.stop_self().ok();
    }
    Ok(())
  }
}
