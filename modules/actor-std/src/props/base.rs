use std::{
  ops::{Deref, DerefMut},
  string::String,
};

use fraktor_actor_core_rs::{
  mailbox::MailboxPolicy,
  props::{ActorFactory, MailboxConfig, PropsGeneric as CorePropsGeneric},
};
use fraktor_utils_core_rs::sync::ArcShared;
use fraktor_utils_std_rs::runtime_toolbox::StdToolbox;

use crate::{
  actor_prim::{Actor, ActorAdapter},
  system::DispatcherConfig,
};

/// Actor properties specialised for `StdToolbox` with a closure ergonomics layer.
#[derive(Clone)]
pub struct Props {
  inner: CorePropsGeneric<StdToolbox>,
}

impl Props {
  /// Creates new props from the provided factory.
  #[must_use]
  pub fn new(factory: ArcShared<dyn ActorFactory<StdToolbox>>) -> Self {
    Self { inner: CorePropsGeneric::new(factory) }
  }

  /// Convenience helper to build props from a closure returning a [`Actor`].
  #[must_use]
  pub fn from_fn<F, A>(factory: F) -> Self
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + Sync + 'static, {
    let wrapped_factory = move || ActorAdapter::new(factory());
    Self { inner: CorePropsGeneric::from_fn(wrapped_factory) }
  }

  /// Returns the actor factory.
  #[must_use]
  pub fn factory(&self) -> &ArcShared<dyn ActorFactory<StdToolbox>> {
    self.inner.factory()
  }

  /// Returns the configured actor name, if any.
  #[must_use]
  pub fn name(&self) -> Option<&str> {
    self.inner.name()
  }

  /// Returns the mailbox configuration.
  #[must_use]
  pub fn mailbox(&self) -> &MailboxConfig {
    self.inner.mailbox()
  }

  /// Returns the mailbox policy.
  #[must_use]
  pub fn mailbox_policy(&self) -> MailboxPolicy {
    self.inner.mailbox_policy()
  }

  /// Returns the registered middleware identifiers.
  #[must_use]
  pub fn middleware(&self) -> &[String] {
    self.inner.middleware()
  }

  /// Returns the configured dispatcher settings.
  #[must_use]
  pub fn dispatcher(&self) -> DispatcherConfig {
    DispatcherConfig::from_core(self.inner.dispatcher().clone())
  }

  /// Updates the mailbox configuration.
  #[must_use]
  pub fn with_mailbox(mut self, config: MailboxConfig) -> Self {
    self.inner = self.inner.with_mailbox(config);
    self
  }

  /// Registers middleware identifiers used when constructing the message pipeline.
  #[must_use]
  pub fn with_middleware<I, S>(mut self, middleware: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>, {
    self.inner = self.inner.with_middleware(middleware);
    self
  }

  /// Assigns a logical name to the actor for registry purposes.
  #[must_use]
  pub fn with_name(mut self, name: impl Into<String>) -> Self {
    self.inner = self.inner.with_name(name);
    self
  }

  /// Overrides the dispatcher configuration used when constructing actors.
  #[must_use]
  pub fn with_dispatcher(mut self, dispatcher: DispatcherConfig) -> Self {
    self.inner = self.inner.with_dispatcher(dispatcher.into_core());
    self
  }

  /// Borrows the underlying core props reference.
  #[must_use]
  pub fn as_core(&self) -> &CorePropsGeneric<StdToolbox> {
    &self.inner
  }

  /// Borrows the underlying core props mutably.
  #[must_use]
  pub fn as_core_mut(&mut self) -> &mut CorePropsGeneric<StdToolbox> {
    &mut self.inner
  }

  /// Consumes the wrapper and returns the underlying core props.
  #[must_use]
  pub fn into_inner(self) -> CorePropsGeneric<StdToolbox> {
    self.inner
  }
}

impl Deref for Props {
  type Target = CorePropsGeneric<StdToolbox>;

  fn deref(&self) -> &Self::Target {
    self.as_core()
  }
}

impl DerefMut for Props {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.as_core_mut()
  }
}
