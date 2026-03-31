//! Core trait for actor reference providers.

use crate::core::kernel::actor::{
  actor_path::{ActorPath, ActorPathScheme},
  actor_ref::ActorRef,
  error::ActorError,
};

/// Trait for all ActorRef providers to implement.
///
/// ActorRefProvider is responsible for creating actor references and managing
/// the actor reference lifecycle. Different implementations provide different
/// actor reference semantics:
///
/// - `LocalActorRefProvider`: For local-only actor systems
/// - `TokioActorRefProvider`: For remote actor systems using Tokio TCP transport
/// - `LoopbackActorRefProvider`: For remote actor systems with loopback routing optimization
///
/// This trait is not intended for extension outside of fraktor-rs core.
pub trait ActorRefProvider: Send + Sync {
  /// Returns the URI schemes handled by this provider.
  #[must_use]
  fn supported_schemes(&self) -> &'static [ActorPathScheme];

  /// Creates an actor reference for the provided path.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor reference cannot be created.
  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError>;

  // Future extensions:
  // fn root_guardian(&self) -> ActorRef;
  // fn guardian(&self) -> ActorRef;
  // fn system_guardian(&self) -> ActorRef;
  // fn dead_letters(&self) -> ActorRef;
  // fn temp_path(&self) -> ActorPath;
}
