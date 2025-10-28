use core::marker::PhantomData;

use crate::{
  sync::{
    ArcShared,
    async_mutex_like::{AsyncMutexLike, SpinAsyncMutex},
  },
  v2::collections::stack::{PushOutcome, StackError, backend::AsyncStackBackend},
};

#[cfg(test)]
mod tests;

pub(super) async fn push_shared<T, B, A>(shared: &ArcShared<A>, item: T) -> Result<PushOutcome, StackError>
where
  B: AsyncStackBackend<T>,
  A: AsyncMutexLike<B>, {
  let mut value = Some(item);

  loop {
    let mut guard = <A as AsyncMutexLike<B>>::lock(&**shared).await.map_err(StackError::from)?;

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
      let result = guard.push(value.take().expect("push value already consumed")).await;
      drop(guard);
      return result;
    }
  }
}

pub(super) async fn pop_shared<T, B, A>(shared: &ArcShared<A>) -> Result<T, StackError>
where
  B: AsyncStackBackend<T>,
  A: AsyncMutexLike<B>, {
  loop {
    let mut guard = <A as AsyncMutexLike<B>>::lock(&**shared).await.map_err(StackError::from)?;

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
pub struct AsyncStack<T, B, A = SpinAsyncMutex<B>>
where
  B: AsyncStackBackend<T>,
  A: AsyncMutexLike<B>, {
  inner: ArcShared<A>,
  _pd:   PhantomData<(T, B)>,
}

impl<T, B, A> AsyncStack<T, B, A>
where
  B: AsyncStackBackend<T>,
  A: AsyncMutexLike<B>,
{
  /// Creates a new async stack from the provided shared backend.
  #[must_use]
  pub fn new(shared_backend: ArcShared<A>) -> Self {
    Self { inner: shared_backend, _pd: PhantomData }
  }

  /// Pushes an item onto the stack.
  pub async fn push(&self, item: T) -> Result<PushOutcome, StackError> {
    push_shared::<T, B, A>(&self.inner, item).await
  }

  /// Pops the top item from the stack.
  pub async fn pop(&self) -> Result<T, StackError> {
    pop_shared::<T, B, A>(&self.inner).await
  }

  /// Returns the top item without removing it.
  pub async fn peek(&self) -> Result<Option<T>, StackError>
  where
    T: Clone, {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(StackError::from)?;
    Ok(guard.peek().cloned())
  }

  /// Requests the backend to transition into the closed state.
  pub async fn close(&self) -> Result<(), StackError> {
    let mut guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(StackError::from)?;
    guard.close().await
  }

  /// Returns the number of stored elements.
  #[must_use]
  pub async fn len(&self) -> Result<usize, StackError> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(StackError::from)?;
    Ok(guard.len())
  }

  /// Returns the storage capacity.
  #[must_use]
  pub async fn capacity(&self) -> Result<usize, StackError> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(StackError::from)?;
    Ok(guard.capacity())
  }

  /// Indicates whether the stack is empty.
  #[must_use]
  pub async fn is_empty(&self) -> Result<bool, StackError> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(StackError::from)?;
    Ok(guard.len() == 0)
  }

  /// Indicates whether the stack is full.
  #[must_use]
  pub async fn is_full(&self) -> Result<bool, StackError> {
    let guard = <A as AsyncMutexLike<B>>::lock(&*self.inner).await.map_err(StackError::from)?;
    Ok(guard.len() == guard.capacity())
  }

  /// Provides access to the underlying shared backend.
  #[must_use]
  pub fn shared(&self) -> &ArcShared<A> {
    &self.inner
  }
}
