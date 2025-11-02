use cellactor_actor_core_rs::{Actor, ActorContext, ActorError, AnyMessageView};

pub struct PrinterActor;

impl Actor for PrinterActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(text) = message.downcast_ref::<String>() {
      println!("[printer pid={}] 受信: {}", ctx.pid(), text);
    }
    Ok(())
  }
}
