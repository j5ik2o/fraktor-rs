//! Typed props.

use alloc::string::String;
use core::marker::PhantomData;

use crate::core::{
  kernel::actor::props::Props,
  typed::{
    actor::TypedActor,
    behavior::Behavior,
    dispatcher_selector::DispatcherSelector,
    internal::{BehaviorRunner, TypedActorAdapter},
    mailbox_selector::MailboxSelector,
  },
};

/// Describes how to construct a typed actor for message `M`.
pub struct TypedProps<M>
where
  M: Send + Sync + 'static, {
  props:  Props,
  marker: PhantomData<M>,
}

impl<M> Clone for TypedProps<M>
where
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self { props: self.props.clone(), marker: PhantomData }
  }
}

impl<M> TypedProps<M>
where
  M: Send + Sync + 'static,
{
  /// Builds props from a typed actor factory.
  #[must_use]
  pub fn new<F, A>(factory: F) -> Self
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: TypedActor<M> + 'static, {
    let props = Props::from_fn(move || TypedActorAdapter::<M>::new(factory()));
    Self { props, marker: PhantomData }
  }

  /// Builds props from a typed behavior factory.
  #[must_use]
  pub fn from_behavior_factory<F>(factory: F) -> Self
  where
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    let props = Props::from_fn(move || {
      let behavior = factory();
      TypedActorAdapter::<M>::new(BehaviorRunner::new(behavior))
    });
    Self { props, marker: PhantomData }
  }

  /// Wraps existing props after applying an external typed conversion.
  #[must_use]
  pub const fn from_props(props: Props) -> Self {
    Self { props, marker: PhantomData }
  }

  /// Returns the underlying props.
  #[must_use]
  pub const fn to_untyped(&self) -> &Props {
    &self.props
  }

  /// Consumes the typed props and returns the props.
  #[must_use]
  pub fn into_untyped(self) -> Props {
    self.props
  }

  /// Applies a mapping function to the props and returns a new typed props.
  #[must_use]
  pub fn map_props(self, f: impl FnOnce(Props) -> Props) -> Self {
    Self { props: f(self.props), marker: PhantomData }
  }

  /// Applies a dispatcher selector to configure the dispatcher assignment.
  #[must_use]
  pub fn with_dispatcher_selector(self, selector: DispatcherSelector) -> Self {
    match selector {
      | DispatcherSelector::Default => self,
      | DispatcherSelector::FromConfig(id) => self.map_props(|p| p.with_dispatcher_id(id)),
      | DispatcherSelector::SameAsParent => self.map_props(|p| p.with_dispatcher_same_as_parent()),
      | DispatcherSelector::Blocking => self
        .map_props(|p| p.with_dispatcher_id(crate::core::kernel::dispatch::dispatcher::DEFAULT_BLOCKING_DISPATCHER_ID)),
    }
  }

  /// Applies a mailbox selector to configure the mailbox assignment.
  #[must_use]
  pub fn with_mailbox_selector(self, selector: MailboxSelector) -> Self {
    match selector {
      | MailboxSelector::Default => self,
      | MailboxSelector::Bounded(capacity) => {
        let policy = crate::core::kernel::dispatch::mailbox::MailboxPolicy::bounded(
          capacity,
          crate::core::kernel::dispatch::mailbox::MailboxOverflowStrategy::DropNewest,
          None,
        );
        let config = crate::core::kernel::actor::props::MailboxConfig::new(policy);
        self.map_props(|p| p.with_mailbox_config(config))
      },
      | MailboxSelector::FromConfig(id) => self.map_props(|p| p.with_mailbox_id(id)),
    }
  }

  /// Shorthand: use the default dispatcher.
  #[must_use]
  pub fn with_dispatcher_default(self) -> Self {
    self.with_dispatcher_selector(DispatcherSelector::Default)
  }

  /// Shorthand: use a dispatcher resolved from configuration.
  #[must_use]
  pub fn with_dispatcher_from_config(self, id: impl Into<String>) -> Self {
    self.with_dispatcher_selector(DispatcherSelector::from_config(id))
  }

  /// Shorthand: use the same dispatcher as the parent actor.
  #[must_use]
  pub fn with_dispatcher_same_as_parent(self) -> Self {
    self.with_dispatcher_selector(DispatcherSelector::SameAsParent)
  }

  /// Shorthand: use a bounded mailbox with the given capacity.
  #[must_use]
  pub fn with_mailbox_bounded(self, capacity: core::num::NonZeroUsize) -> Self {
    self.with_mailbox_selector(MailboxSelector::bounded(capacity))
  }

  /// Attaches metadata tags to the actor for observability and routing.
  ///
  /// This mirrors Pekko's `ActorTags`.
  #[must_use]
  pub fn with_tags<I, S>(self, tags: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<alloc::string::String>, {
    self.map_props(|p| p.with_tags(tags))
  }

  /// Adds a single metadata tag to the actor.
  #[must_use]
  pub fn with_tag(self, tag: impl Into<alloc::string::String>) -> Self {
    self.map_props(|p| p.with_tag(tag))
  }
}
