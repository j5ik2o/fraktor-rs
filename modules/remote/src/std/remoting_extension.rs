//! Standard-library wiring for remoting extension lifecycle.

use alloc::{format, string::ToString};

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  system::{ActorSystemGeneric, SystemGuardianProtocol},
};
use fraktor_utils_rs::{
  core::{
    runtime_toolbox::{RuntimeToolbox, SyncMutexFamily},
    sync::{ArcShared, sync_mutex_like::SyncMutexLike},
  },
  std::runtime_toolbox::StdToolbox,
};

use crate::{
  core::{
    RemotingControl, RemotingControlHandle, RemotingControlShared, RemotingError, RemotingExtensionConfig,
    RemotingExtensionGeneric, transport::RemoteTransportShared,
  },
  std::transport::StdTransportFactory,
};

const ENDPOINT_SUPERVISOR_NAME: &str = "remoting-endpoint-supervisor";

/// Standard library extension implementation specialized for [`StdToolbox`].
///
/// This implementation supports all transport schemes including Tokio TCP.
impl RemotingExtensionGeneric<StdToolbox> {
  /// Creates and wires the extension, panicking on unrecoverable errors.
  #[must_use]
  pub fn new(system: &ActorSystemGeneric<StdToolbox>, config: &RemotingExtensionConfig) -> Self {
    Self::try_new(system, config).unwrap_or_else(|error| panic!("failed to initialize remoting extension: {error}"))
  }

  /// Attempts to install the extension, returning an error if invariants are violated.
  pub fn try_new(
    system: &ActorSystemGeneric<StdToolbox>,
    config: &RemotingExtensionConfig,
  ) -> Result<Self, RemotingError> {
    let control_handle = RemotingControlHandle::new(system.clone(), config.clone());
    let control: RemotingControlShared<StdToolbox> =
      ArcShared::new(<<StdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(control_handle));
    let mut transport = StdTransportFactory::build(config)?;
    transport.install_backpressure_hook(control.lock().backpressure_hook());
    let shared_transport: RemoteTransportShared<StdToolbox> = RemoteTransportShared::new(transport);
    control.lock().register_remote_transport_shared(shared_transport.clone());
    let mut guardian = system.system_guardian_ref().ok_or(RemotingError::SystemGuardianUnavailable)?;
    let supervisor = spawn_endpoint_supervisor(system, &guardian, control.clone())?;
    register_shutdown_hook(&mut guardian, &supervisor)?;
    if config.auto_start() {
      control.lock().start()?;
    }
    Ok(Self { control, transport_scheme: config.transport_scheme().to_string(), _transport: shared_transport })
  }
}

fn spawn_endpoint_supervisor<TB>(
  system: &ActorSystemGeneric<TB>,
  guardian: &ActorRefGeneric<TB>,
  control: RemotingControlShared<TB>,
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
  guardian: &mut ActorRefGeneric<TB>,
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
  control:  RemotingControlShared<TB>,
  guardian: ActorRefGeneric<TB>,
}

impl<TB> EndpointSupervisorActor<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn new(control: RemotingControlShared<TB>, guardian: ActorRefGeneric<TB>) -> Self {
    Self { control, guardian }
  }

  fn acknowledge_shutdown(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    self.control.lock().notify_system_shutdown();
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
