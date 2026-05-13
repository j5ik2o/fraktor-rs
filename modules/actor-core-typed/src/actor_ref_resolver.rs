//! Serialization and deserialization helpers for typed actor references.

#[cfg(test)]
#[path = "actor_ref_resolver_test.rs"]
mod tests;

use alloc::string::String;

use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_path::ActorPathParser, actor_ref::ActorRef, actor_ref_provider::ActorRefResolveError, extension::Extension,
  },
  system::{ActorSystem, ActorSystemWeak},
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{TypedActorRef, TypedActorSystem, internal::ActorRefResolverId};

/// Resolves typed actor references to and from canonical string representations.
#[derive(Clone)]
pub struct ActorRefResolver {
  system: ActorSystemWeak,
}

impl ActorRefResolver {
  /// Creates a resolver bound to the provided actor system.
  #[must_use]
  pub fn new(system: &ActorSystem) -> Self {
    Self { system: system.downgrade() }
  }

  /// Returns the resolver extension registered for the provided typed system.
  #[must_use]
  pub fn get<M>(system: &TypedActorSystem<M>) -> Option<ArcShared<Self>>
  where
    M: Send + Sync + 'static, {
    system.as_untyped().extended().extension(&ActorRefResolverId::new())
  }

  pub(crate) fn install(system: &ActorSystem) {
    let id = ActorRefResolverId::new();
    if system.extended().has_extension(&id) {
      return;
    }
    let registered = system.extended().register_extension(&id);
    if let Some(existing) = system.extended().extension(&id) {
      debug_assert!(ArcShared::ptr_eq(&registered, &existing));
    }
  }

  /// Serializes an untyped actor reference into a canonical string form.
  ///
  /// # Errors
  ///
  /// Returns [`ActorRefResolveError::NotFound`] when the actor path is unavailable.
  pub fn to_serialization_format(&self, actor_ref: &ActorRef) -> Result<String, ActorRefResolveError> {
    let system = self.system.upgrade().ok_or(ActorRefResolveError::SystemNotBootstrapped)?;
    if let Some(actor_system) = actor_ref.system_state()
      && !system.state().ptr_eq(&actor_system)
    {
      return Err(ActorRefResolveError::NotFound(
        "actor ref belongs to another actor system; use that system's resolver".into(),
      ));
    }
    if let Some(path) = actor_ref.canonical_path() {
      return Ok(path.to_canonical_uri());
    }
    if let Some(path) = actor_ref.path() {
      let canonical = system.state().canonical_actor_path(&actor_ref.pid()).unwrap_or(path);
      return Ok(canonical.to_canonical_uri());
    }
    Err(ActorRefResolveError::NotFound("actor path unavailable for serialization".into()))
  }

  /// Serializes a typed actor reference into a canonical string form.
  ///
  /// # Errors
  ///
  /// Returns [`ActorRefResolveError`] when the actor path is unavailable.
  pub fn to_serialization_format_typed<M>(&self, actor_ref: &TypedActorRef<M>) -> Result<String, ActorRefResolveError>
  where
    M: Send + Sync + 'static, {
    self.to_serialization_format(actor_ref.as_untyped())
  }

  /// Resolves an untyped actor reference from the serialized format produced by this resolver.
  ///
  /// # Errors
  ///
  /// Returns [`ActorRefResolveError`] when parsing or lookup fails.
  pub fn resolve_actor_ref(&self, serialized_actor_ref: &str) -> Result<ActorRef, ActorRefResolveError> {
    let system = self.system.upgrade().ok_or(ActorRefResolveError::SystemNotBootstrapped)?;
    let path = ActorPathParser::parse(serialized_actor_ref)
      .map_err(|error| ActorRefResolveError::NotFound(alloc::format!("invalid actor ref format: {error:?}")))?;
    if let Some(pid) = system.pid_by_path(&path)
      && let Some(actor_ref) = system.actor_ref_by_pid(pid)
    {
      return Ok(actor_ref);
    }
    system.resolve_actor_ref(path)
  }

  /// Resolves a typed actor reference from the serialized format produced by this resolver.
  ///
  /// This first performs the untyped lookup via [`Self::resolve_actor_ref`] and
  /// then converts the result with [`TypedActorRef::from_untyped`]. That
  /// conversion performs no runtime verification of `M`, so callers must ensure
  /// the resolved actor actually accepts the requested message type.
  ///
  /// # Errors
  ///
  /// Returns [`ActorRefResolveError`] when parsing or lookup fails.
  pub fn resolve_typed_actor_ref<M>(
    &self,
    serialized_actor_ref: &str,
  ) -> Result<TypedActorRef<M>, ActorRefResolveError>
  where
    M: Send + Sync + 'static, {
    self.resolve_actor_ref(serialized_actor_ref).map(TypedActorRef::from_untyped)
  }
}

impl Extension for ActorRefResolver {}
