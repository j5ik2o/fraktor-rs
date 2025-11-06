use cellactor_actor_core_rs::actor_prim::Pid;
use crate::typed::actor_prim::TypedActorContext;
use cellactor_actor_core_rs::error::ActorError;

pub trait TypedActor<M>: Send + Sync
where
  M: Send + Sync + 'static, {
  fn pre_start(&mut self, _ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    Ok(())
  }
  fn receive(&mut self, _ctx: &mut TypedActorContext<'_, M>, _message: &M) -> Result<(), ActorError> {
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut TypedActorContext<'_, M>) -> Result<(), ActorError> {
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut TypedActorContext<'_, M>, _terminated: Pid) -> Result<(), ActorError> {
    Ok(())
  }

}
