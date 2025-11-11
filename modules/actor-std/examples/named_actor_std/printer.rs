use fraktor_actor_core_rs::error::ActorError;
use fraktor_actor_std_rs::{
  actor_prim::{Actor, ActorContext},
  messaging::AnyMessageView,
};

pub struct PrinterActor;

impl Actor for PrinterActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(text) = message.downcast_ref::<String>() {
      println!("[printer pid={}] 受信: {}", ctx.pid(), text);
    }
    Ok(())
  }
}
