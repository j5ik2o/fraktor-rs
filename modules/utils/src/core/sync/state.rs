use core::ops::{Deref, DerefMut};

#[cfg(test)]
mod tests;

/// Trait abstracting a state holder that expects exclusive access from callers.
///
/// This trait no longer relies on interior mutability. Callers must guarantee
/// exclusive access (e.g., by holding an external lock) before invoking
/// mutating methods. Implementations can therefore use plain mutable references
/// internally without embedding synchronization primitives.
///
/// # Design Philosophy
///
/// - **Abstraction**: Hides implementation details, enabling the same code to work across different
///   runtime environments
/// - **Flexibility**: Lets callers combine the state holder with external synchronization that
///   suits the environment (e.g., single-threaded tests, system-level mutexes)
/// - **Type Safety**: Leverages Generic Associated Types (GAT) to guarantee type safety at compile
///   time
///
/// # Example Implementation
///
/// ```rust
/// # use core::ops::{Deref, DerefMut};
/// # pub trait StateCell<T>: Clone {
/// #   type Ref<'a>: Deref<Target = T> where Self: 'a, T: 'a;
/// #   type RefMut<'a>: DerefMut<Target = T> where Self: 'a, T: 'a;
/// #   fn new(value: T) -> Self where Self: Sized;
/// #   fn borrow(&mut self) -> Self::Ref<'_>;
/// #   fn borrow_mut(&mut self) -> Self::RefMut<'_>;
/// # }
///
/// // Minimal implementation that stores the value directly.
/// #[derive(Clone)]
/// struct InlineState<T: Clone>(T);
///
/// impl<T: Clone> StateCell<T> for InlineState<T> {
///   type Ref<'a>
///     = &'a T
///   where
///     Self: 'a,
///     T: 'a;
///   type RefMut<'a>
///     = &'a mut T
///   where
///     Self: 'a,
///     T: 'a;
///
///   fn new(value: T) -> Self {
///     Self(value)
///   }
///
///   fn borrow(&mut self) -> Self::Ref<'_> {
///     &self.0
///   }
///
///   fn borrow_mut(&mut self) -> Self::RefMut<'_> {
///     &mut self.0
///   }
/// }
/// ```
pub trait StateCell<T>: Clone {
  /// Immutable reference guard type.
  ///
  /// Functions as an RAII type implementing `Deref<Target = T>` that automatically
  /// releases the lock when it goes out of scope. Depending on the runtime implementation,
  /// different types such as `Ref<'a, T>`, `MutexGuard<'a, T>`, `RwLockReadGuard<'a, T>` are used.
  type Ref<'a>: Deref<Target = T>
  where
    Self: 'a,
    T: 'a;

  /// Mutable reference guard type.
  ///
  /// Functions as an RAII type implementing `DerefMut<Target = T>` that automatically
  /// releases the lock when it goes out of scope. Depending on the runtime implementation,
  /// different types such as `RefMut<'a, T>`, `MutexGuard<'a, T>`, `RwLockWriteGuard<'a, T>` are
  /// used.
  type RefMut<'a>: DerefMut<Target = T>
  where
    Self: 'a,
    T: 'a;

  /// Constructs a new state cell with the specified value.
  ///
  /// # Arguments
  ///
  /// * `value` - Value to store as initial state
  ///
  /// # Returns
  ///
  /// Newly created state cell instance
  fn new(value: T) -> Self
  where
    Self: Sized;

  /// Borrows the state immutably.
  ///
  /// This method returns a guard type that provides read-only access to the internal state.
  /// The lock is automatically released when the guard goes out of scope.
  ///
  /// # Returns
  ///
  /// Guard object holding an immutable reference to the state
  ///
  /// # Panics
  ///
  /// Depending on the implementation, may panic if a mutable borrow already exists
  /// (e.g., `RefCell`-based implementations).
  fn borrow(&mut self) -> Self::Ref<'_>;

  /// Borrows the state mutably.
  ///
  /// This method returns a guard type that provides read-write access to the internal state.
  /// The lock is automatically released when the guard goes out of scope.
  ///
  /// # Returns
  ///
  /// Guard object holding a mutable reference to the state
  ///
  /// # Panics
  ///
  /// Depending on the implementation, may panic if any borrow already exists
  /// (e.g., `RefCell`-based implementations).
  fn borrow_mut(&mut self) -> Self::RefMut<'_>;

  /// Executes a closure with an immutable reference to the state.
  ///
  /// This method borrows the state and passes the reference to the closure for execution.
  /// The lock is automatically released when the closure completes.
  /// Enables safer and more concise code by eliminating the need to manually manage guards.
  ///
  /// # Arguments
  ///
  /// * `f` - Closure that receives an immutable reference to the state and returns a value of type
  ///   `R`
  ///
  /// # Returns
  ///
  /// Result of executing the closure
  fn with_ref<R>(&mut self, f: impl FnOnce(&T) -> R) -> R {
    let guard = self.borrow();
    f(&*guard)
  }

  /// Executes a closure with a mutable reference to the state.
  ///
  /// This method mutably borrows the state and passes the reference to the closure for execution.
  /// The lock is automatically released when the closure completes.
  /// Enables safer and more concise code by eliminating the need to manually manage guards.
  ///
  /// # Arguments
  ///
  /// * `f` - Closure that receives a mutable reference to the state and returns a value of type `R`
  ///
  /// # Returns
  ///
  /// Result of executing the closure
  fn with_ref_mut<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
    let mut guard = self.borrow_mut();
    f(&mut *guard)
  }
}
