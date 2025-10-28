use super::{
  StackError,
  traits::{StackBackend, StackBase, StackHandle, StackMut},
};
use crate::collections::QueueSize;

#[cfg(test)]
mod tests;

/// Stack facade that delegates operations to [`StackBackend`].
///
/// This struct abstracts stack operations and provides a unified interface
/// to the actual backend implementation.
///
/// # Type Parameters
///
/// * `H` - Backend handle type implementing [`StackHandle`]
/// * `T` - Type of elements stored in the stack
#[derive(Debug)]
pub struct Stack<H, T>
where
  H: StackHandle<T>, {
  backend: H,
  _marker: core::marker::PhantomData<T>,
}

impl<H, T> Stack<H, T>
where
  H: StackHandle<T>,
{
  /// Creates a new [`Stack`] from the specified backend handle.
  ///
  /// # Arguments
  ///
  /// * `backend` - Backend handle to process stack operations
  ///
  /// # Returns
  ///
  /// A new [`Stack`] instance
  #[must_use]
  pub const fn new(backend: H) -> Self {
    Self { backend, _marker: core::marker::PhantomData }
  }

  /// Gets a reference to the backend handle.
  ///
  /// # Returns
  ///
  /// Immutable reference to the backend handle
  pub const fn backend(&self) -> &H {
    &self.backend
  }

  /// Consumes the [`Stack`] and extracts the internal backend handle.
  ///
  /// # Returns
  ///
  /// Internal backend handle
  pub fn into_backend(self) -> H {
    self.backend
  }

  /// Sets the stack's capacity limit.
  ///
  /// # Arguments
  ///
  /// * `capacity` - Maximum capacity; unlimited if `None`
  pub fn set_capacity(&self, capacity: Option<usize>) {
    self.backend.backend().set_capacity(capacity);
  }

  /// Pushes a value onto the top of the stack.
  ///
  /// Returns [`StackError::Full`] if the stack is full and a capacity limit is set.
  ///
  /// # Arguments
  ///
  /// * `value` - Value to push
  ///
  /// # Returns
  ///
  /// * `Ok(())` - Push succeeded
  /// * `Err(StackError<T>)` - Stack is full; error contains the original value
  ///
  /// # Errors
  ///
  /// * `StackError::Full(value)` - Stack is full; error contains the original value
  pub fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.backend.backend().push(value)
  }

  /// Pops a value from the top of the stack.
  ///
  /// # Returns
  ///
  /// * `Some(T)` - Popped value
  /// * `None` - If stack is empty
  pub fn pop(&self) -> Option<T> {
    self.backend.backend().pop()
  }

  /// References the top value of the stack without removing it.
  ///
  /// This operation clones the element, so `T` must implement [`Clone`].
  ///
  /// # Returns
  ///
  /// * `Some(T)` - Clone of the top value
  /// * `None` - If stack is empty
  pub fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.backend.backend().peek()
  }

  /// Clears all elements in the stack.
  pub fn clear(&self) {
    self.backend.backend().clear();
  }

  /// Gets the number of elements stored in the stack.
  ///
  /// # Returns
  ///
  /// [`QueueSize`] representing the current element count
  pub fn len(&self) -> QueueSize {
    self.backend.backend().len()
  }

  /// Gets the capacity of the stack.
  ///
  /// # Returns
  ///
  /// * `QueueSize::Bounded(n)` - Maximum capacity is `n`
  /// * `QueueSize::Unbounded` - Unlimited
  pub fn capacity(&self) -> QueueSize {
    self.backend.backend().capacity()
  }
}

impl<H, T> Clone for Stack<H, T>
where
  H: StackHandle<T>,
{
  /// Creates a clone of the [`Stack`].
  ///
  /// Clones the backend handle to create a new instance that references
  /// the same stack.
  fn clone(&self) -> Self {
    Self { backend: self.backend.clone(), _marker: core::marker::PhantomData }
  }
}

/// Implementation of [`StackBase`] trait.
///
/// Provides basic stack query operations (retrieving length and capacity).
impl<H, T> StackBase<T> for Stack<H, T>
where
  H: StackHandle<T>,
{
  fn len(&self) -> QueueSize {
    self.backend.backend().len()
  }

  fn capacity(&self) -> QueueSize {
    self.backend.backend().capacity()
  }
}

/// Implementation of [`StackMut`] trait.
///
/// Provides methods to manipulate the stack through mutable references.
/// Allows the same operations as immutable methods to be performed with a `&mut self` receiver.
impl<H, T> StackMut<T> for Stack<H, T>
where
  H: StackHandle<T>,
{
  /// # Errors
  ///
  /// * `StackError::Full(value)` - Stack is full; error contains the original value
  fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    self.backend.backend().push(value)
  }

  fn pop(&mut self) -> Option<T> {
    self.backend.backend().pop()
  }

  fn clear(&mut self) {
    self.backend.backend().clear();
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.backend.backend().peek()
  }
}
