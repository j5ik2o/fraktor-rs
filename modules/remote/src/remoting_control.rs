//! Control interface exposed to runtime operators.

use alloc::vec::Vec;

use fraktor_actor_rs::core::actor_prim::actor_path::ActorPathParts;
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::{
  RemotingBackpressureListener, RemotingConnectionSnapshot, RemotingError,
};

/// High-level control surface for remoting subsystems.
pub trait RemotingControl<TB: RuntimeToolbox + 'static>: Send + Sync + Clone + 'static {
  /// Starts the remoting subsystem.
  fn start(&self) -> Result<(), RemotingError>;

  /// Initiates an association to the provided remote address.
  fn associate(&self, address: &ActorPathParts) -> Result<(), RemotingError>;

  /// Quarantines the specified authority for the supplied reason.
  fn quarantine(&self, authority: &str, reason: &str) -> Result<(), RemotingError>;

  /// Initiates a graceful shutdown sequence.
  fn shutdown(&self) -> Result<(), RemotingError>;

  /// Registers a listener interested in backpressure signals.
  fn register_backpressure_listener(&self, listener: ArcShared<dyn RemotingBackpressureListener>);

  /// Returns a snapshot view of current remote authorities.
  fn connections_snapshot(&self) -> Vec<RemotingConnectionSnapshot>;
}
