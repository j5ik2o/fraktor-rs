//! High level control plane for remoting subsystems.

use alloc::vec::Vec;

use fraktor_actor_rs::core::actor_prim::actor_path::ActorPathParts;
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  quarantine_reason::QuarantineReason, remote_authority_snapshot::RemoteAuthoritySnapshot,
  remoting_backpressure_listener::RemotingBackpressureListener, remoting_error::RemotingError,
};

/// Public API exposed to extensions, providers, and system services to orchestrate remoting.
pub trait RemotingControl<TB>: Send + Sync
where
  TB: RuntimeToolbox + 'static, {
  /// Starts the remoting subsystem if it is not already running.
  fn start(&self) -> Result<(), RemotingError>;

  /// Requests an association with the provided actor-path authority.
  fn associate(&self, address: &ActorPathParts) -> Result<(), RemotingError>;

  /// Forces the specified authority into quarantine for the provided reason.
  fn quarantine(&self, authority: &str, reason: &QuarantineReason) -> Result<(), RemotingError>;

  /// Initiates shutdown and releases transport resources.
  fn shutdown(&self) -> Result<(), RemotingError>;

  /// Registers a listener for future backpressure notifications.
  fn register_backpressure_listener<L>(&self, listener: L)
  where
    L: RemotingBackpressureListener;

  /// Returns a snapshot of all known authorities.
  fn connections_snapshot(&self) -> Vec<RemoteAuthoritySnapshot>;
}
