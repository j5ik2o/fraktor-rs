//! Minimal endpoint supervisor actor registered under the system guardian.

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  spawn::SpawnError,
  system::{ActorSystemGeneric, SystemGuardianProtocol},
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::RemotingControlHandle;

pub(crate) struct EndpointSupervisor<TB: RuntimeToolbox + 'static> {
  #[allow(dead_code)]
  control: RemotingControlHandle<TB>,
}

impl<TB: RuntimeToolbox + 'static> EndpointSupervisor<TB> {
  pub(crate) fn new(control: RemotingControlHandle<TB>) -> Self {
    Self { control }
  }

  pub(crate) fn spawn(
    system: &ActorSystemGeneric<TB>,
    control: RemotingControlHandle<TB>,
  ) -> Result<ActorRefGeneric<TB>, SpawnError> {
    let props = PropsGeneric::from_fn({
      let control = control.clone();
      move || Self::new(control.clone())
    })
    .with_name("remoting-endpoint-supervisor");
    let child = system.spawn_system_actor(&props)?;
    Ok(child.actor_ref().clone())
  }
}

impl<TB: RuntimeToolbox + 'static> Actor<TB> for EndpointSupervisor<TB> {
  fn pre_start(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    self.register_termination_hook(ctx)
  }

  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(protocol) = _message.downcast_ref::<SystemGuardianProtocol<TB>>() {
      self.handle_protocol(_ctx, protocol)
    } else {
      Ok(())
    }
  }
}

impl<TB: RuntimeToolbox + 'static> EndpointSupervisor<TB> {
  fn register_termination_hook(&self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    let Some(actor_ref) = ctx.system().system_guardian_ref() else {
      return Err(ActorError::recoverable("system guardian unavailable"));
    };
    actor_ref
      .tell(AnyMessageGeneric::new(SystemGuardianProtocol::<TB>::RegisterTerminationHook(ctx.self_ref())))
      .map_err(|error| ActorError::from_send_error(&error))
  }

  fn handle_protocol(
    &self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    protocol: &SystemGuardianProtocol<TB>,
  ) -> Result<(), ActorError> {
    match protocol {
      | SystemGuardianProtocol::TerminationHook => {
        self.control.publish_shutdown();
        if let Some(actor_ref) = ctx.system().system_guardian_ref() {
          actor_ref
            .tell(AnyMessageGeneric::new(SystemGuardianProtocol::<TB>::TerminationHookDone(ctx.self_ref())))
            .map_err(|error| ActorError::from_send_error(&error))?;
        }
        Ok(())
      },
      | _ => Ok(()),
    }
  }
}
