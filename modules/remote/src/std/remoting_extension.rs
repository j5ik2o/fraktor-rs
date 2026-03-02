//! Standard-library wiring for remoting extension lifecycle.

use alloc::{format, string::ToString};

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContext, actor_ref::ActorRef},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  system::{ActorSystem, guardian::SystemGuardianProtocol},
};
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::{
  core::{
    remoting_extension::{
      RemotingControl, RemotingControlHandle, RemotingControlShared, RemotingError, RemotingExtension,
      RemotingExtensionConfig,
    },
    transport::RemoteTransportShared,
  },
  std::transport::StdTransportFactory,
};

const ENDPOINT_SUPERVISOR_NAME: &str = "remoting-endpoint-supervisor";

/// Standard library extension implementation.
///
/// This implementation supports all transport schemes including Tokio TCP.
impl RemotingExtension {
  /// Creates and wires the extension, panicking on unrecoverable errors.
  #[must_use]
  pub fn new(system: &ActorSystem, config: &RemotingExtensionConfig) -> Self {
    Self::try_new(system, config).unwrap_or_else(|error| panic!("failed to initialize remoting extension: {error}"))
  }

  /// Attempts to install the extension, returning an error if invariants are violated.
  pub fn try_new(system: &ActorSystem, config: &RemotingExtensionConfig) -> Result<Self, RemotingError> {
    let control_handle = RemotingControlHandle::new(system.clone(), config.clone());
    let control: RemotingControlShared = ArcShared::new(RuntimeMutex::new(control_handle));
    let mut transport = StdTransportFactory::build(config)?;
    transport.install_backpressure_hook(control.lock().backpressure_hook());
    let shared_transport: RemoteTransportShared = RemoteTransportShared::new(transport);
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

fn spawn_endpoint_supervisor(
  system: &ActorSystem,
  guardian: &ActorRef,
  control: RemotingControlShared,
) -> Result<ActorRef, RemotingError> {
  let props = Props::from_fn({
    let handle = control.clone();
    let guardian_ref = guardian.clone();
    move || EndpointSupervisorActor::new(handle.clone(), guardian_ref.clone())
  })
  .with_name(ENDPOINT_SUPERVISOR_NAME);
  let child = system.extended().spawn_system_actor(&props).map_err(RemotingError::from)?;
  Ok(child.actor_ref().clone())
}

fn register_shutdown_hook(guardian: &mut ActorRef, supervisor: &ActorRef) -> Result<(), RemotingError> {
  guardian
    .tell(AnyMessage::new(SystemGuardianProtocol::RegisterTerminationHook(supervisor.clone())))
    .map_err(|error| RemotingError::HookRegistrationFailed(format!("{error:?}")))
}

struct EndpointSupervisorActor {
  control:  RemotingControlShared,
  guardian: ActorRef,
}

impl EndpointSupervisorActor {
  fn new(control: RemotingControlShared, guardian: ActorRef) -> Self {
    Self { control, guardian }
  }

  fn acknowledge_shutdown(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.control.lock().notify_system_shutdown();
    self
      .guardian
      .tell(AnyMessage::new(SystemGuardianProtocol::TerminationHookDone(ctx.self_ref())))
      .map_err(|error| ActorError::from_send_error(&error))
  }
}

impl Actor for EndpointSupervisorActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(protocol) = message.downcast_ref::<SystemGuardianProtocol>()
      && matches!(protocol, SystemGuardianProtocol::TerminationHook)
    {
      self.acknowledge_shutdown(ctx)?;
    }
    Ok(())
  }
}
