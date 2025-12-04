//! High level control plane for remoting subsystems.

use alloc::vec::Vec;

use fraktor_actor_rs::core::actor_prim::actor_path::ActorPathParts;
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  quarantine_reason::QuarantineReason, remote_authority_snapshot::RemoteAuthoritySnapshot,
  remoting_backpressure_listener::RemotingBackpressureListener, remoting_control_handle::RemotingControlHandle,
  remoting_error::RemotingError,
};

/// Public API exposed to extensions, providers, and system services to orchestrate remoting.
pub trait RemotingControl<TB>: Send + Sync
where
  TB: RuntimeToolbox + 'static, {
  /// Starts the remoting subsystem if it is not already running.
  fn start(&mut self) -> Result<(), RemotingError>;

  /// Requests an association with the provided actor-path authority.
  fn associate(&mut self, address: &ActorPathParts) -> Result<(), RemotingError>;

  /// Forces the specified authority into quarantine for the provided reason.
  fn quarantine(&mut self, authority: &str, reason: &QuarantineReason) -> Result<(), RemotingError>;

  /// Initiates shutdown and releases transport resources.
  fn shutdown(&mut self) -> Result<(), RemotingError>;

  /// Registers a listener for future backpressure notifications.
  fn register_backpressure_listener<L>(&mut self, listener: L)
  where
    L: RemotingBackpressureListener;

  /// Returns a snapshot of all known authorities.
  fn connections_snapshot(&self) -> Vec<RemoteAuthoritySnapshot>;
}

/// Shared handle wrapping [`RemotingControlHandle`] with external synchronization.
pub type RemotingControlShared<TB> =
  ArcShared<fraktor_utils_rs::core::runtime_toolbox::ToolboxMutex<RemotingControlHandle<TB>, TB>>;
