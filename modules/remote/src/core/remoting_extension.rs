//! Extension entry point wiring remoting control and supervisor actors.

use alloc::format;

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  extension::Extension,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  system::{ActorSystemGeneric, SystemGuardianProtocol},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  remoting_control::RemotingControl,
  remoting_control_handle::RemotingControlHandle,
  remoting_error::RemotingError,
  remoting_extension_config::RemotingExtensionConfig,
  transport::{RemoteTransport, TransportFactory},
};

const ENDPOINT_SUPERVISOR_NAME: &str = "remoting-endpoint-supervisor";

/// Installs the endpoint supervisor and exposes [`RemotingControlHandle`].
pub struct RemotingExtension<TB>
where
  TB: RuntimeToolbox + 'static, {
  control:    RemotingControlHandle<TB>,
  _transport: ArcShared<dyn RemoteTransport>,
}

impl<TB> RemotingExtension<TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Creates and wires the extension, panicking on unrecoverable errors.
  #[must_use]
  pub fn new(system: &ActorSystemGeneric<TB>, config: &RemotingExtensionConfig) -> Self {
    Self::try_new(system, config).unwrap_or_else(|error| panic!("failed to initialize remoting extension: {error}"))
  }

  /// Attempts to install the extension, returning an error if invariants are violated.
  pub fn try_new(system: &ActorSystemGeneric<TB>, config: &RemotingExtensionConfig) -> Result<Self, RemotingError> {
    let control = RemotingControlHandle::new(system.clone(), config.clone());
    let transport = TransportFactory::build(config)?;
    transport.install_backpressure_hook(control.backpressure_hook());
    let guardian = system.system_guardian_ref().ok_or(RemotingError::SystemGuardianUnavailable)?;
    let supervisor = spawn_endpoint_supervisor(system, &guardian, control.clone())?;
    register_shutdown_hook(&guardian, &supervisor)?;
    if config.auto_start() {
      control.start()?;
    }
    Ok(Self { control, _transport: transport })
  }

  /// Returns the shared control handle.
  #[must_use]
  pub fn handle(&self) -> RemotingControlHandle<TB> {
    self.control.clone()
  }
}

impl<TB> Extension<TB> for RemotingExtension<TB> where TB: RuntimeToolbox + 'static {}

fn spawn_endpoint_supervisor<TB>(
  system: &ActorSystemGeneric<TB>,
  guardian: &ActorRefGeneric<TB>,
  control: RemotingControlHandle<TB>,
) -> Result<ActorRefGeneric<TB>, RemotingError>
where
  TB: RuntimeToolbox + 'static, {
  let props = PropsGeneric::from_fn({
    let handle = control.clone();
    let guardian_ref = guardian.clone();
    move || EndpointSupervisorActor::new(handle.clone(), guardian_ref.clone())
  })
  .with_name(ENDPOINT_SUPERVISOR_NAME);
  let child = system.extended().spawn_system_actor(&props).map_err(RemotingError::from)?;
  Ok(child.actor_ref().clone())
}

fn register_shutdown_hook<TB>(
  guardian: &ActorRefGeneric<TB>,
  supervisor: &ActorRefGeneric<TB>,
) -> Result<(), RemotingError>
where
  TB: RuntimeToolbox + 'static, {
  guardian
    .tell(AnyMessageGeneric::new(SystemGuardianProtocol::RegisterTerminationHook(supervisor.clone())))
    .map_err(|error| RemotingError::HookRegistrationFailed(format!("{error:?}")))
}

struct EndpointSupervisorActor<TB>
where
  TB: RuntimeToolbox + 'static, {
  control:  RemotingControlHandle<TB>,
  guardian: ActorRefGeneric<TB>,
}

impl<TB> EndpointSupervisorActor<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn new(control: RemotingControlHandle<TB>, guardian: ActorRefGeneric<TB>) -> Self {
    Self { control, guardian }
  }

  fn acknowledge_shutdown(&self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    self.control.notify_system_shutdown();
    self
      .guardian
      .tell(AnyMessageGeneric::new(SystemGuardianProtocol::TerminationHookDone(ctx.self_ref())))
      .map_err(|error| ActorError::from_send_error(&error))
  }
}

impl<TB> Actor<TB> for EndpointSupervisorActor<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(protocol) = message.downcast_ref::<SystemGuardianProtocol<TB>>()
      && matches!(protocol, SystemGuardianProtocol::TerminationHook)
    {
      self.acknowledge_shutdown(ctx)?;
    }
    Ok(())
  }
}
