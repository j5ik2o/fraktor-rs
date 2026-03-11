use fraktor_actor_rs::{
  core::{error::ActorError, messaging::AnyMessageView},
  std::actor::{Actor, ActorContext},
};

pub struct PrinterActor;

impl Actor for PrinterActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(text) = message.downcast_ref::<String>() {
      println!("[printer pid={}] 受信: {}", ctx.pid(), text);
    }
    Ok(())
  }
}
