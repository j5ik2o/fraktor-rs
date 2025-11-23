//! Extended ActorSystem API surface for infrastructure components.

use core::any::Any;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use super::{ActorRefProvider, ActorSystemGeneric, RegisterExtraTopLevelError, RemoteWatchHook};
use crate::core::{
  actor_prim::{ChildRefGeneric, actor_ref::ActorRefGeneric},
  dispatcher::DispatchersGeneric,
  extension::{Extension, ExtensionId},
  mailbox::MailboxesGeneric,
  props::PropsGeneric,
  spawn::SpawnError,
};

/// Provides privileged operations required by extensions and system daemons.
#[derive(Clone)]
pub struct ExtendedActorSystemGeneric<TB: RuntimeToolbox + 'static> {
  inner: ActorSystemGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> ExtendedActorSystemGeneric<TB> {
  /// Creates a new extended wrapper around the provided [`ActorSystemGeneric`].
  #[must_use]
  pub const fn new(inner: ActorSystemGeneric<TB>) -> Self {
    Self { inner }
  }

  /// Returns the underlying actor system reference.
  #[must_use]
  pub const fn actor_system(&self) -> &ActorSystemGeneric<TB> {
    &self.inner
  }

  /// Converts the wrapper back into the actor system.
  #[must_use]
  pub fn into_actor_system(self) -> ActorSystemGeneric<TB> {
    self.inner
  }

  /// Returns the dispatcher registry.
  #[must_use]
  pub fn dispatchers(&self) -> ArcShared<DispatchersGeneric<TB>> {
    self.inner.state().dispatchers()
  }

  /// Returns the mailbox registry.
  #[must_use]
  pub fn mailboxes(&self) -> ArcShared<MailboxesGeneric<TB>> {
    self.inner.state().mailboxes()
  }

  /// Registers the provided extension and returns the shared instance.
  pub fn register_extension<E>(&self, ext_id: &E) -> ArcShared<E::Ext>
  where
    E: ExtensionId<TB>, {
    let state = self.inner.state();
    state.extension_or_insert_with(ext_id.id(), || ArcShared::new(ext_id.create_extension(self.actor_system())))
  }

  /// Retrieves a previously registered extension.
  #[must_use]
  pub fn extension<E>(&self, ext_id: &E) -> Option<ArcShared<E::Ext>>
  where
    E: ExtensionId<TB>, {
    self.inner.state().extension(ext_id.id())
  }

  /// Returns `true` when the extension has already been registered.
  #[must_use]
  pub fn has_extension<E>(&self, ext_id: &E) -> bool
  where
    E: ExtensionId<TB>, {
    self.inner.state().has_extension(ext_id.id())
  }

  /// Returns the extension instance by concrete type.
  #[must_use]
  pub fn extension_by_type<E>(&self) -> Option<ArcShared<E>>
  where
    E: Extension<TB> + 'static, {
    self.inner.state().extension_by_type::<E>()
  }

  /// Registers an actor-ref provider for later retrieval.
  pub fn register_actor_ref_provider<P>(&self, provider: &ArcShared<P>)
  where
    P: ActorRefProvider<TB> + Any + Send + Sync + 'static, {
    self.inner.state().install_actor_ref_provider(provider);
  }

  /// Returns the actor-ref provider of the requested type when registered.
  #[must_use]
  pub fn actor_ref_provider<P>(&self) -> Option<ArcShared<P>>
  where
    P: Any + Send + Sync + 'static, {
    self.inner.state().actor_ref_provider::<P>()
  }

  /// Registers a remote watch hook that intercepts watch/unwatch to remote actors.
  pub fn register_remote_watch_hook<H>(&self, hook: ArcShared<H>)
  where
    H: RemoteWatchHook<TB>, {
    let dyn_hook: ArcShared<dyn RemoteWatchHook<TB>> = hook;
    self.inner.state().register_remote_watch_hook(dyn_hook);
  }

  /// Registers an extra top-level actor name before the system finishes startup.
  ///
  /// # Errors
  ///
  /// Returns [`RegisterExtraTopLevelError`] if the name is reserved, duplicated, or registration
  /// occurs after startup.
  pub fn register_extra_top_level(
    &self,
    name: &str,
    actor: ActorRefGeneric<TB>,
  ) -> Result<(), RegisterExtraTopLevelError> {
    self.inner.state().register_extra_top_level(name, actor)
  }

  /// Spawns a new actor as a child of the system guardian (extensions/internal subsystems).
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::SystemUnavailable`] when the system guardian is missing.
  pub fn spawn_system_actor(&self, props: &PropsGeneric<TB>) -> Result<ChildRefGeneric<TB>, SpawnError> {
    self.inner.system_actor_of(props)
  }
}

/// Type alias for [`ExtendedActorSystemGeneric`] using the default [`NoStdToolbox`].
pub type ExtendedActorSystem = ExtendedActorSystemGeneric<NoStdToolbox>;
