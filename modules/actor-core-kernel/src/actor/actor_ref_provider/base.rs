//! Core trait for actor reference providers.

use alloc::{format, string::String};

use crate::{
  actor::{
    Address,
    actor_path::{ActorPath, ActorPathParser, ActorPathParts, ActorPathScheme, GuardianKind},
    actor_ref::ActorRef,
    deploy::Deployer,
    error::ActorError,
  },
  system::TerminationSignal,
};

#[cfg(test)]
mod tests;

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

  /// Returns the root guardian actor when available.
  #[must_use]
  fn root_guardian(&self) -> Option<ActorRef> {
    None
  }

  /// Returns the user guardian actor when available.
  #[must_use]
  fn guardian(&self) -> Option<ActorRef> {
    None
  }

  /// Returns the system guardian actor when available.
  #[must_use]
  fn system_guardian(&self) -> Option<ActorRef> {
    None
  }

  /// Returns the dead-letters sink actor.
  #[must_use]
  fn dead_letters(&self) -> ActorRef {
    ActorRef::null()
  }

  /// Returns the base temporary actor path used by this provider.
  #[must_use]
  fn temp_path(&self) -> ActorPath {
    ActorPath::root().child("temp")
  }

  /// Returns the local guardian root path handled by this provider.
  #[must_use]
  fn root_path(&self) -> ActorPath {
    ActorPath::from_parts(ActorPathParts::local("cellactor").with_guardian(GuardianKind::User))
  }

  /// Returns the root guardian ref for the requested address when reachable.
  #[must_use]
  fn root_guardian_at(&self, _address: &Address) -> Option<ActorRef> {
    None
  }

  /// Returns the deployer registry associated with this provider.
  #[must_use]
  fn deployer(&self) -> Option<Deployer> {
    None
  }

  /// Resolves an actor reference for the provided path.
  ///
  /// # Errors
  ///
  /// Returns an error when the provider cannot resolve the path.
  fn resolve_actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.actor_ref(path)
  }

  /// Resolves an actor reference from its canonical string representation.
  ///
  /// # Errors
  ///
  /// Returns an error when the string is not a valid actor path or when
  /// resolution fails.
  fn resolve_actor_ref_str(&mut self, path: &str) -> Result<ActorRef, ActorError> {
    let path =
      ActorPathParser::parse(path).map_err(|error| ActorError::fatal(format!("invalid actor path: {error}")))?;
    self.resolve_actor_ref(path)
  }

  /// Returns a generated temporary actor path using the provided prefix hint.
  ///
  /// # Errors
  ///
  /// Returns an error when the provider cannot generate a valid prefixed temp path.
  fn temp_path_with_prefix(&self, _prefix: &str) -> Result<ActorPath, ActorError> {
    Err(ActorError::fatal("temporary prefixed actor path is not supported by this provider"))
  }

  /// Returns the actor reference representing `/temp` when available.
  #[must_use]
  fn temp_container(&self) -> Option<ActorRef> {
    None
  }

  /// Registers a temporary actor reference and returns the generated segment name.
  fn register_temp_actor(&self, _actor: ActorRef) -> Option<String> {
    None
  }

  /// Removes a temporary actor mapping if present.
  fn unregister_temp_actor(&self, _name: &str) {}

  /// Unregisters a temporary actor using its `/temp/...` path.
  ///
  /// # Errors
  ///
  /// Returns an error when the path is not a valid temp actor path.
  fn unregister_temp_actor_path(&self, _path: &ActorPath) -> Result<(), ActorError> {
    Err(ActorError::fatal("temporary actor path unregistration is not supported by this provider"))
  }

  /// Resolves a temporary actor reference by generated segment name.
  #[must_use]
  fn temp_actor(&self, _name: &str) -> Option<ActorRef> {
    None
  }

  /// Returns a signal that resolves when the backing actor system terminates.
  #[must_use]
  fn termination_signal(&self) -> TerminationSignal;

  /// Returns the external address to use when communicating with the given address.
  #[must_use]
  fn get_external_address_for(&self, _addr: &Address) -> Option<Address> {
    None
  }

  /// Returns the default external address for this provider when available.
  #[must_use]
  fn get_default_address(&self) -> Option<Address> {
    None
  }
}
