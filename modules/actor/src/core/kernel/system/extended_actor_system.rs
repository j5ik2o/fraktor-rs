//! Extended ActorSystem API surface for infrastructure components.

use alloc::boxed::Box;
use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

use super::{
  ActorSystem, ActorSystemBuildError, RegisterExtensionError, RegisterExtraTopLevelError, remote::RemoteWatchHook,
};
use crate::core::kernel::{
  actor::{
      ChildRef,
      actor_ref::ActorRef,
      extension::{Extension, ExtensionId},
      props::{MailboxConfig, Props},
      actor_ref_provider::{ActorRefProvider, ActorRefProviderShared},
      spawn::SpawnError,
  },
  dispatch::{
    dispatcher::{DispatcherConfig, DispatcherRegistryError},
    mailbox::MailboxRegistryError,
  },
};

/// Provides privileged operations required by extensions and system daemons.
#[derive(Clone)]
pub struct ExtendedActorSystem {
  inner: ActorSystem,
}

impl ExtendedActorSystem {
  /// Creates a new extended wrapper around the provided [`ActorSystem`].
  #[must_use]
  pub const fn new(inner: ActorSystem) -> Self {
    Self { inner }
  }

  /// Returns the underlying actor system reference.
  #[must_use]
  pub const fn actor_system(&self) -> &ActorSystem {
    &self.inner
  }

  /// Converts the wrapper back into the actor system.
  #[must_use]
  pub fn into_actor_system(self) -> ActorSystem {
    self.inner
  }

  /// Resolves the dispatcher configuration for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DispatcherRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve_dispatcher(&self, id: &str) -> Result<DispatcherConfig, DispatcherRegistryError> {
    self.inner.state().resolve_dispatcher(id)
  }

  /// Resolves the mailbox configuration for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve_mailbox(&self, id: &str) -> Result<MailboxConfig, MailboxRegistryError> {
    self.inner.state().resolve_mailbox(id)
  }

  /// Registers the provided extension and returns the shared instance.
  ///
  /// # Errors
  ///
  /// Returns [`RegisterExtensionError::AlreadyStarted`] when the actor system already finished
  /// startup and the extension is not registered yet.
  pub fn register_extension<E>(&self, ext_id: &E) -> Result<ArcShared<E::Ext>, RegisterExtensionError>
  where
    E: ExtensionId, {
    let state = self.inner.state();
    state.extension_or_insert_with(ext_id.id(), || ArcShared::new(ext_id.create_extension(self.actor_system())))
  }

  /// Retrieves a previously registered extension.
  #[must_use]
  pub fn extension<E>(&self, ext_id: &E) -> Option<ArcShared<E::Ext>>
  where
    E: ExtensionId, {
    self.inner.state().extension(ext_id.id())
  }

  /// Returns `true` when the extension has already been registered.
  #[must_use]
  pub fn has_extension<E>(&self, ext_id: &E) -> bool
  where
    E: ExtensionId, {
    self.inner.state().has_extension(ext_id.id())
  }

  /// Returns the extension instance by concrete type.
  #[must_use]
  pub fn extension_by_type<E>(&self) -> Option<ArcShared<E>>
  where
    E: Extension + 'static, {
    self.inner.state().extension_by_type::<E>()
  }

  /// Registers an actor-ref provider for later retrieval.
  ///
  /// # Errors
  ///
  /// Returns [`ActorSystemBuildError::Configuration`] when called after system startup.
  pub fn register_actor_ref_provider<P>(
    &self,
    provider: &ActorRefProviderShared<P>,
  ) -> Result<(), ActorSystemBuildError>
  where
    P: ActorRefProvider + Any + Send + Sync + 'static, {
    self.inner.state().install_actor_ref_provider(provider)
  }

  /// Returns the actor-ref provider of the requested type when registered.
  #[must_use]
  pub fn actor_ref_provider<P>(&self) -> Option<ActorRefProviderShared<P>>
  where
    P: ActorRefProvider + Any + Send + Sync + 'static, {
    self.inner.state().actor_ref_provider::<P>()
  }

  /// Registers a remote watch hook that intercepts watch/unwatch to remote actors.
  ///
  /// The hook will be wrapped in a `RuntimeMutex` internally for thread-safe access.
  pub fn register_remote_watch_hook<H>(&self, hook: H)
  where
    H: RemoteWatchHook, {
    let dyn_hook: Box<dyn RemoteWatchHook> = Box::new(hook);
    self.inner.state().register_remote_watch_hook(dyn_hook);
  }

  /// Registers an extra top-level actor name before the system finishes startup.
  ///
  /// # Errors
  ///
  /// Returns [`RegisterExtraTopLevelError`] if the name is reserved, duplicated, or registration
  /// occurs after startup.
  pub fn register_extra_top_level(&self, name: &str, actor: ActorRef) -> Result<(), RegisterExtraTopLevelError> {
    self.inner.state().register_extra_top_level(name, actor)
  }

  /// Spawns a new actor as a child of the system guardian (extensions/internal subsystems).
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::SystemUnavailable`] when the system guardian is missing.
  pub fn spawn_system_actor(&self, props: &Props) -> Result<ChildRef, SpawnError> {
    self.inner.system_actor_of(props)
  }
}
