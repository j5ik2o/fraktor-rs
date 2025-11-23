//! Core trait for actor reference providers.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  actor_prim::{
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRefGeneric,
  },
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
pub trait ActorRefProvider<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Returns the URI schemes handled by this provider.
  #[must_use]
  fn supported_schemes(&self) -> &'static [ActorPathScheme];

  /// Creates an actor reference for the provided path.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor reference cannot be created.
  fn actor_ref(&self, path: ActorPath) -> Result<ActorRefGeneric<TB>, ActorError>;

  // Future extensions:
  // fn root_guardian(&self) -> ActorRefGeneric<TB>;
  // fn guardian(&self) -> ActorRefGeneric<TB>;
  // fn system_guardian(&self) -> ActorRefGeneric<TB>;
  // fn dead_letters(&self) -> ActorRefGeneric<TB>;
  // fn temp_path(&self) -> ActorPath;
}
