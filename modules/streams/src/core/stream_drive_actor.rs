use alloc::{collections::BTreeMap, vec::Vec};

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{StreamHandleGeneric, StreamHandleId, stream_drive_command::StreamDriveCommand};

/// Actor that drives registered streams on each tick.
pub(crate) struct StreamDriveActor<TB: RuntimeToolbox + 'static> {
  handles:            BTreeMap<StreamHandleId, StreamHandleGeneric<TB>>,
  shutdown_requested: bool,
}

impl<TB: RuntimeToolbox + 'static> StreamDriveActor<TB> {
  pub(crate) const fn new() -> Self {
    Self { handles: BTreeMap::new(), shutdown_requested: false }
  }

  fn register(&mut self, handle: StreamHandleGeneric<TB>) {
    if self.shutdown_requested {
      let _ = handle.cancel();
      return;
    }
    self.handles.insert(handle.id(), handle);
  }

  fn tick(&mut self) {
    let mut finished = Vec::new();
    for (id, handle) in self.handles.iter() {
      let _ = handle.drive();
      if handle.state().is_terminal() {
        finished.push(*id);
      }
    }
    for id in finished {
      self.handles.remove(&id);
    }
  }

  fn shutdown(&mut self) {
    for handle in self.handles.values() {
      let _ = handle.cancel();
    }
    self.handles.clear();
    self.shutdown_requested = true;
  }

  fn try_stop(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    if self.shutdown_requested && self.handles.is_empty() {
      ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))
    } else {
      Ok(())
    }
  }
}

impl<TB: RuntimeToolbox + 'static> Actor<TB> for StreamDriveActor<TB> {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<StreamDriveCommand<TB>>() {
      match command {
        | StreamDriveCommand::Register { handle } => {
          self.register(handle.clone());
        },
        | StreamDriveCommand::Tick => {
          self.tick();
        },
        | StreamDriveCommand::Shutdown => {
          self.shutdown();
        },
      }
    }
    self.try_stop(ctx)
  }
}
