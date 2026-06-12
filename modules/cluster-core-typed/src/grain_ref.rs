//! Typed grain reference wrapper.

#[cfg(test)]
#[path = "grain_ref_test.rs"]
mod tests;

use core::{any::Any, marker::PhantomData};

use fraktor_actor_core_kernel_rs::actor::{actor_ref::ActorRef, messaging::AnyMessage};
use fraktor_actor_core_typed_rs::{
  TypedActorRef,
  dsl::{TypedAskFuture, TypedAskResponse},
};
use fraktor_cluster_core_kernel_rs::grain::{GrainCallError, GrainCallOptions, GrainCodec, GrainRef as KernelGrainRef};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::ClusterIdentity;

/// Typed facade over the kernel grain reference for message type `M`.
///
/// This is the fraktor equivalent of Pekko's `EntityRef[M]`. It wraps a
/// [`KernelGrainRef`](fraktor_cluster_core_kernel_rs::grain::GrainRef) and
/// provides type-safe [`tell_with_sender`](Self::tell_with_sender),
/// [`request`](Self::request), and [`request_future`](Self::request_future)
/// methods that only accept messages of type `M`.
///
/// # Type assertions
///
/// The response type `R` in [`request`](Self::request) and
/// [`request_future`](Self::request_future) is asserted, not verified at
/// construction time. A mismatch is detected via
/// [`TypedAskError::TypeMismatch`](fraktor_actor_core_typed_rs::dsl::TypedAskError::TypeMismatch)
/// when the caller attempts to take the resolved value. This is equivalent to
/// the runtime-checked semantics of Pekko's `ask`.
///
/// # Single-consumer futures
///
/// The [`TypedAskFuture`](fraktor_actor_core_typed_rs::dsl::TypedAskFuture)
/// returned by [`request_future`](Self::request_future) is intended for a
/// single consumer. Cloning the future and calling
/// [`try_take`](fraktor_actor_core_typed_rs::dsl::TypedAskFuture::try_take)
/// from multiple holders may return
/// [`TypedAskError::SharedReferences`](fraktor_actor_core_typed_rs::dsl::TypedAskError::SharedReferences).
///
/// # Conversion
///
/// Conversion between typed and untyped references is **explicit only**:
/// use [`from_kernel`](Self::from_kernel), [`as_kernel`](Self::as_kernel), and
/// [`into_kernel`](Self::into_kernel). `From` / `Into` implementations are
/// intentionally absent (requirement 4.3).
///
/// # Compile-time type safety
///
/// Attempting to call [`tell_with_sender`](Self::tell_with_sender) with a
/// value of the wrong message type is a compile error:
///
/// ```compile_fail
/// use fraktor_cluster_core_typed_rs::{ClusterIdentity, GrainRef};
/// use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;
///
/// // 型 M = u32 の GrainRef に String を送ろうとするとコンパイルエラーになる。
/// fn wrong_type(grain: &GrainRef<u32>, sender: &fraktor_actor_core_typed_rs::TypedActorRef<()>) {
///     let _ = grain.tell_with_sender(String::from("wrong"), sender);
/// }
/// ```
pub struct GrainRef<M> {
  inner:    KernelGrainRef,
  _message: PhantomData<fn() -> M>,
}

impl<M> GrainRef<M>
where
  M: Any + Send + Sync + 'static,
{
  /// Wraps an untyped grain reference with an asserted message type `M`.
  ///
  /// No type verification is performed at construction time. The assertion
  /// is enforced at the call site through the typed `tell_with_sender`,
  /// `request`, and `request_future` methods.
  #[must_use]
  pub const fn from_kernel(inner: KernelGrainRef) -> Self {
    Self { inner, _message: PhantomData }
  }

  /// Returns a reference to the underlying untyped grain reference.
  #[must_use]
  pub const fn as_kernel(&self) -> &KernelGrainRef {
    &self.inner
  }

  /// Consumes this wrapper and returns the underlying untyped grain reference.
  #[must_use]
  pub fn into_kernel(self) -> KernelGrainRef {
    self.inner
  }

  /// Returns the typed cluster identity of this grain reference.
  #[must_use]
  pub fn identity(&self) -> ClusterIdentity<M> {
    ClusterIdentity::from_kernel(self.inner.identity().clone())
  }

  /// Returns a reference with the given call options applied.
  ///
  /// The options are passed through to the underlying kernel reference.
  #[must_use]
  pub fn with_options(self, options: GrainCallOptions) -> Self {
    Self { inner: self.inner.with_options(options), _message: PhantomData }
  }

  /// Returns a reference with the given codec applied.
  ///
  /// The codec is passed through to the underlying kernel reference.
  #[must_use]
  pub fn with_codec(self, codec: ArcShared<dyn GrainCodec>) -> Self {
    Self { inner: self.inner.with_codec(codec), _message: PhantomData }
  }

  /// Sends a fire-and-forget message with an explicit sender.
  ///
  /// The message is wrapped into [`AnyMessage`] and delegated to the kernel.
  ///
  /// # Errors
  ///
  /// Returns a [`GrainCallError`] if resolution or sending fails.
  pub fn tell_with_sender<S>(&self, message: M, sender: &TypedActorRef<S>) -> Result<(), GrainCallError>
  where
    S: Send + Sync + 'static, {
    let any_msg = AnyMessage::new(message);
    let untyped_sender: &ActorRef = sender.as_untyped();
    self.inner.tell_with_sender(&any_msg, untyped_sender)
  }

  /// Sends a request and returns a typed response handle.
  ///
  /// The response type `R` is asserted. A type mismatch between the actual
  /// reply and `R` is detected via
  /// [`TypedAskError::TypeMismatch`](fraktor_actor_core_typed_rs::dsl::TypedAskError::TypeMismatch)
  /// when the caller takes the value from the future.
  ///
  /// # Errors
  ///
  /// Returns a [`GrainCallError`] if resolution or sending fails.
  pub fn request<R>(&self, message: M) -> Result<TypedAskResponse<R>, GrainCallError>
  where
    R: Send + Sync + 'static, {
    let any_msg = AnyMessage::new(message);
    let response = self.inner.request(&any_msg)?;
    Ok(TypedAskResponse::from_untyped(response))
  }

  /// Sends a request and returns a typed response future.
  ///
  /// The future resolves with `Ok(R)` on success or `Err(TypedAskError)` on
  /// failure. The future is intended for a **single consumer**; cloning and
  /// calling `try_take` from multiple holders may yield
  /// [`TypedAskError::SharedReferences`](fraktor_actor_core_typed_rs::dsl::TypedAskError::SharedReferences).
  ///
  /// # Errors
  ///
  /// Returns a [`GrainCallError`] if resolution or sending fails.
  pub fn request_future<R>(&self, message: M) -> Result<TypedAskFuture<R>, GrainCallError>
  where
    R: Send + Sync + 'static, {
    let any_msg = AnyMessage::new(message);
    let future = self.inner.request_future(&any_msg)?;
    Ok(TypedAskFuture::from_untyped(future))
  }
}
