//! Extended ActorSystem API surface for infrastructure components.

use alloc::{boxed::Box, string::ToString};
use core::any::Any;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{ActorSystem, ActorSystemBuildError, RegisterExtraTopLevelError, remote::RemoteWatchHook};
use crate::core::kernel::{
  actor::{
    ChildRef,
    actor_path::ActorPath,
    actor_ref::ActorRef,
    actor_ref_provider::{ActorRefProvider, ActorRefProviderShared},
    actor_selection::ActorSelection,
    error::SendError,
    extension::{Extension, ExtensionId},
    props::{MailboxConfig, Props},
    spawn::SpawnError,
  },
  dispatch::{
    dispatcher::{DispatchersError, MessageDispatcherShared},
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

  /// Resolves a [`MessageDispatcherShared`] for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchersError::Unknown`] when the identifier has not been
  /// registered in the dispatcher registry.
  pub fn resolve_dispatcher(&self, id: &str) -> Result<MessageDispatcherShared, DispatchersError> {
    self.inner.state().resolve_dispatcher(id).ok_or_else(|| DispatchersError::Unknown(ToString::to_string(id)))
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
  /// Registers an extension or returns the existing one (putIfAbsent semantics).
  pub fn register_extension<E>(&self, ext_id: &E) -> ArcShared<E::Ext>
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

  /// Spawns a new top-level actor under the user guardian.
  ///
  /// Corresponds to classic `ActorRefFactory.actorOf(props)`.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the actor cannot be created.
  pub fn actor_of(&self, props: &Props) -> Result<ChildRef, SpawnError> {
    self.inner.actor_of(props)
  }

  /// Spawns a new named top-level actor under the user guardian.
  ///
  /// Corresponds to classic `ActorRefFactory.actorOf(props, name)`.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the actor cannot be created, including duplicate names.
  pub fn actor_of_named(&self, props: &Props, name: &str) -> Result<ChildRef, SpawnError> {
    self.inner.actor_of_named(props, name)
  }

  /// Sends a stop signal to the specified actor reference.
  ///
  /// Corresponds to classic `ActorRefFactory.stop(actor)`.
  ///
  /// # Errors
  ///
  /// Returns an error if the stop message cannot be enqueued.
  pub fn stop(&self, actor: &ActorRef) -> Result<(), SendError> {
    self.inner.stop(actor)
  }

  /// Creates a classic actor selection rooted at the actor system.
  #[must_use]
  pub fn actor_selection(&self, path: &str) -> ActorSelection {
    self.inner.actor_selection(path)
  }

  /// Creates a classic actor selection anchored to the provided path.
  #[must_use]
  pub fn actor_selection_from_path(&self, path: &ActorPath) -> ActorSelection {
    self.inner.actor_selection_from_path(path)
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
