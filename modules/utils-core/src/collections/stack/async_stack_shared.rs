use core::marker::PhantomData;

use super::AsyncStack;
use crate::{
  collections::stack::{PushOutcome, StackError, backend::AsyncStackBackend},
  sync::{
    ArcShared,
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
  },
};

#[cfg(test)]
mod tests;

pub(crate) async fn push_shared<T, B, A>(shared: &ArcShared<A>, item: T) -> Result<PushOutcome, StackError>
where
  B: AsyncStackBackend<T>,
  A: AsyncMutexLike<AsyncStack<T, B>>, {
  let mut value = Some(item);

  loop {
    let mut guard = <A as AsyncMutexLike<AsyncStack<T, B>>>::lock(&**shared).await.map_err(StackError::from)?;

    if guard.is_closed() {
      return Err(StackError::Closed);
    }

    if guard.is_full() {
      if let Some(waiter) = guard.prepare_push_wait() {
        drop(guard);

        match waiter.await {
          | Ok(()) => continue,
          | Err(err) => return Err(err),
        }
      } else {
        drop(guard);
        return Err(StackError::Full);
      }
    } else {
      let Some(item) = value.take() else {
        return Err(StackError::Closed);
      };
      let result = guard.push(item).await;
      drop(guard);
      return result;
    }
  }
}

pub(crate) async fn pop_shared<T, B, A>(shared: &ArcShared<A>) -> Result<T, StackError>
where
  B: AsyncStackBackend<T>,
  A: AsyncMutexLike<AsyncStack<T, B>>, {
  loop {
    let mut guard = <A as AsyncMutexLike<AsyncStack<T, B>>>::lock(&**shared).await.map_err(StackError::from)?;

    if guard.is_empty() {
      if guard.is_closed() {
        drop(guard);
        return Err(StackError::Closed);
      }

      if let Some(waiter) = guard.prepare_pop_wait() {
        drop(guard);

        match waiter.await {
          | Ok(()) => continue,
          | Err(err) => return Err(err),
        }
      } else {
        drop(guard);
        return Err(StackError::Empty);
      }
    } else {
      let result = guard.pop().await;
      drop(guard);
      return result;
    }
  }
}

/// Async stack API wrapping a shared backend guarded by an async-capable mutex.
#[derive(Clone)]
pub struct AsyncStackShared<T, B, A = SpinAsyncMutex<AsyncStack<T, B>>>
where
  B: AsyncStackBackend<T>,
  A: AsyncMutexLike<AsyncStack<T, B>>, {
  inner: ArcShared<A>,
  _pd:   PhantomData<(T, B)>,
}

impl<T, B, A> AsyncStackShared<T, B, A>
where
  B: AsyncStackBackend<T>,
  A: AsyncMutexLike<AsyncStack<T, B>>,
{
  /// Creates a new async stack from the provided shared backend.
  #[must_use]
  pub const fn new(shared_backend: ArcShared<A>) -> Self {
    Self { inner: shared_backend, _pd: PhantomData }
  }

  /// Pushes an item onto the stack.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend rejects the item because the stack is closed or at
  /// capacity.
  pub async fn push(&self, item: T) -> Result<PushOutcome, StackError> {
    push_shared::<T, B, A>(&self.inner, item).await
  }

  /// Pops the top item from the stack.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend cannot supply an item due to closure or disconnection.
  pub async fn pop(&self) -> Result<T, StackError> {
    pop_shared::<T, B, A>(&self.inner).await
  }

  /// Returns the top item without removing it.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend cannot access the top element.
  pub async fn peek(&self) -> Result<Option<T>, StackError>
  where
    T: Clone, {
    let guard = <A as AsyncMutexLike<AsyncStack<T, B>>>::lock(&*self.inner).await.map_err(StackError::from)?;
    Ok(guard.peek().ok().flatten())
  }

  /// Requests the backend to transition into the closed state.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend refuses to close.
  pub async fn close(&self) -> Result<(), StackError> {
    let mut guard = <A as AsyncMutexLike<AsyncStack<T, B>>>::lock(&*self.inner).await.map_err(StackError::from)?;
    guard.close().await;
    Ok(())
  }

  /// Returns the number of stored elements.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend cannot report its length.
  pub async fn len(&self) -> Result<usize, StackError> {
    let guard = <A as AsyncMutexLike<AsyncStack<T, B>>>::lock(&*self.inner).await.map_err(StackError::from)?;
    Ok(guard.len())
  }

  /// Returns the storage capacity.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend cannot expose its capacity.
  pub async fn capacity(&self) -> Result<usize, StackError> {
    let guard = <A as AsyncMutexLike<AsyncStack<T, B>>>::lock(&*self.inner).await.map_err(StackError::from)?;
    Ok(guard.capacity())
  }

  /// Indicates whether the stack is empty.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend cannot determine emptiness.
  pub async fn is_empty(&self) -> Result<bool, StackError> {
    let guard = <A as AsyncMutexLike<AsyncStack<T, B>>>::lock(&*self.inner).await.map_err(StackError::from)?;
    Ok(guard.is_empty())
  }

  /// Indicates whether the stack is full.
  ///
  /// # Errors
  ///
  /// Returns a `StackError` when the backend cannot determine fullness.
  pub async fn is_full(&self) -> Result<bool, StackError> {
    let guard = <A as AsyncMutexLike<AsyncStack<T, B>>>::lock(&*self.inner).await.map_err(StackError::from)?;
    Ok(guard.is_full())
  }

  /// Provides access to the underlying shared backend.
  #[must_use]
  pub const fn shared(&self) -> &ArcShared<A> {
    &self.inner
  }
}
