//! Extension entry point wiring remoting control and supervisor actors.
#![allow(cfg_std_forbid)]

use alloc::string::String;
#[cfg(feature = "std")]
use alloc::{format, string::ToString};

use fraktor_actor_rs::core::extension::Extension;
#[cfg(feature = "std")]
use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  system::{ActorSystemGeneric, SystemGuardianProtocol},
};
#[cfg(not(feature = "std"))]
use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};
#[cfg(feature = "std")]
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};
#[cfg(feature = "std")]
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

#[cfg(not(feature = "std"))]
use crate::core::{remoting_control::RemotingControlShared, transport::RemoteTransportShared};
#[cfg(feature = "std")]
use crate::core::{
  remoting_control::{RemotingControl, RemotingControlShared},
  remoting_control_handle::RemotingControlHandle,
  remoting_error::RemotingError,
  remoting_extension_config::RemotingExtensionConfig,
  transport::{RemoteTransportShared, TransportFactory},
};

#[cfg(feature = "std")]
const ENDPOINT_SUPERVISOR_NAME: &str = "remoting-endpoint-supervisor";

/// Installs the endpoint supervisor and exposes [`RemotingControlHandle`].
pub struct RemotingExtensionGeneric<TB>
where
  TB: RuntimeToolbox + 'static, {
  control:          RemotingControlShared<TB>,
  transport_scheme: String,
  _transport:       RemoteTransportShared<TB>,
}

/// Type alias for `RemotingExtensionGeneric` with the default `NoStdToolbox`.
pub type RemotingExtension = RemotingExtensionGeneric<NoStdToolbox>;

impl<TB> RemotingExtensionGeneric<TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Returns the shared control handle.
  #[must_use]
  pub fn handle(&self) -> RemotingControlShared<TB> {
    self.control.clone()
  }

  /// Returns the configured transport scheme.
  #[must_use]
  pub fn transport_scheme(&self) -> &str {
    &self.transport_scheme
  }
}

/// Standard library extension implementation specialized for [`StdToolbox`].
///
/// This implementation supports all transport schemes including Tokio TCP.
#[cfg(feature = "std")]
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
    let mut transport = TransportFactory::build(config)?;
    transport.install_backpressure_hook(control.lock().backpressure_hook());
    let shared_transport: RemoteTransportShared<StdToolbox> = RemoteTransportShared::new(transport);
    control.lock().register_remote_transport_shared(shared_transport.clone());
    let guardian = system.system_guardian_ref().ok_or(RemotingError::SystemGuardianUnavailable)?;
    let supervisor = spawn_endpoint_supervisor(system, &guardian, control.clone())?;
    register_shutdown_hook(&guardian, &supervisor)?;
    if config.auto_start() {
      control.lock().start()?;
    }
    Ok(Self { control, transport_scheme: config.transport_scheme().to_string(), _transport: shared_transport })
  }
}

impl<TB> Extension<TB> for RemotingExtensionGeneric<TB> where TB: RuntimeToolbox + 'static {}

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
struct EndpointSupervisorActor<TB>
where
  TB: RuntimeToolbox + 'static, {
  control:  RemotingControlShared<TB>,
  guardian: ActorRefGeneric<TB>,
}

#[cfg(feature = "std")]
impl<TB> EndpointSupervisorActor<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn new(control: RemotingControlShared<TB>, guardian: ActorRefGeneric<TB>) -> Self {
    Self { control, guardian }
  }

  fn acknowledge_shutdown(&self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    self.control.lock().notify_system_shutdown();
    self
      .guardian
      .tell(AnyMessageGeneric::new(SystemGuardianProtocol::TerminationHookDone(ctx.self_ref())))
      .map_err(|error| ActorError::from_send_error(&error))
  }
}

#[cfg(feature = "std")]
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
