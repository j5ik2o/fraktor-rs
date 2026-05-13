//! Builder helpers for typed message adapter registration.

#[cfg(test)]
#[path = "builder_test.rs"]
mod tests;

use core::marker::PhantomData;

use crate::{TypedActorRef, actor::TypedActorContext, message_adapter::AdapterError};

/// Fluent builder for registering an adapter from external payload `U` to actor message `M`.
pub struct MessageAdapterBuilder<'ctx, 'a, M, U>
where
  M: Send + Sync + 'static,
  U: Send + Sync + 'static, {
  context: &'ctx mut TypedActorContext<'a, M>,
  name:    Option<&'ctx str>,
  _marker: PhantomData<U>,
}

impl<'ctx, 'a, M, U> MessageAdapterBuilder<'ctx, 'a, M, U>
where
  M: Send + Sync + 'static,
  U: Send + Sync + 'static,
{
  /// Creates a builder bound to a typed actor context.
  #[must_use]
  pub(crate) const fn new(context: &'ctx mut TypedActorContext<'a, M>) -> Self {
    Self { context, name: None, _marker: PhantomData }
  }

  /// Assigns a logical adapter name for diagnostic intent.
  ///
  /// The current runtime may not consume this name directly, but storing it at call sites
  /// clarifies purpose and keeps compatibility with future named adapter support.
  #[must_use]
  pub const fn with_name(mut self, name: &'ctx str) -> Self {
    self.name = Some(name);
    self
  }

  /// Registers a fallible adapter function.
  ///
  /// # Errors
  ///
  /// Returns [`AdapterError`] when registration fails or when the registry is unavailable.
  pub fn register<F>(self, adapter: F) -> Result<TypedActorRef<U>, AdapterError>
  where
    F: Fn(U) -> Result<M, AdapterError> + Send + Sync + 'static, {
    match self.name {
      | Some(name) => self.context.spawn_message_adapter(Some(name), adapter),
      | None => self.context.message_adapter(adapter),
    }
  }

  /// Registers an infallible mapping from `U` to `M`.
  ///
  /// # Errors
  ///
  /// Returns [`AdapterError`] when registration fails or when the registry is unavailable.
  pub fn register_map<F>(self, mapper: F) -> Result<TypedActorRef<U>, AdapterError>
  where
    F: Fn(U) -> M + Send + Sync + 'static, {
    self.register(move |payload| Ok(mapper(payload)))
  }
}
