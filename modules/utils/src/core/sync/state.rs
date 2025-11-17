use core::ops::{Deref, DerefMut};

#[cfg(test)]
mod tests;

/// Trait abstracting an internal mutable cell for storing actor state.
///
/// This trait is intentionally designed to be lightweight, allowing runtimes
/// to provide implementations using `Rc<RefCell<T>>`, `Arc<Mutex<T>>`, `Arc<RwLock<T>>`, etc.,
/// while enabling state to be referenced and updated through a unified API.
///
/// # Design Philosophy
///
/// - **Abstraction**: Hides implementation details, enabling the same code to work across different
///   runtime environments
/// - **Flexibility**: Allows choosing appropriate implementation for the environment (e.g.,
///   `Rc<RefCell<T>>` for single-threaded, `Arc<Mutex<T>>` for multi-threaded)
/// - **Type Safety**: Leverages Generic Associated Types (GAT) to guarantee type safety at compile
///   time
///
/// # Example Implementation
///
/// ```rust
/// use std::{
///   cell::{Ref, RefCell, RefMut},
///   rc::Rc,
/// };
/// # use core::ops::{Deref, DerefMut};
/// # pub trait StateCell<T>: Clone {
/// #   type Ref<'a>: Deref<Target = T> where Self: 'a, T: 'a;
/// #   type RefMut<'a>: DerefMut<Target = T> where Self: 'a, T: 'a;
/// #   fn new(value: T) -> Self where Self: Sized;
/// #   fn borrow(&self) -> Self::Ref<'_>;
/// #   fn borrow_mut(&self) -> Self::RefMut<'_>;
/// # }
///
/// // Implementation for single-threaded environments
/// struct RcState<T>(Rc<RefCell<T>>);
///
/// impl<T> Clone for RcState<T> {
///   fn clone(&self) -> Self {
///     Self(self.0.clone())
///   }
/// }
///
/// impl<T> StateCell<T> for RcState<T> {
///   type Ref<'a>
///     = Ref<'a, T>
///   where
///     Self: 'a,
///     T: 'a;
///   type RefMut<'a>
///     = RefMut<'a, T>
///   where
///     Self: 'a,
///     T: 'a;
///
///   fn new(value: T) -> Self {
///     Self(Rc::new(RefCell::new(value)))
///   }
///
///   fn borrow(&self) -> Self::Ref<'_> {
///     self.0.borrow()
///   }
///
///   fn borrow_mut(&self) -> Self::RefMut<'_> {
///     self.0.borrow_mut()
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
  fn borrow(&self) -> Self::Ref<'_>;

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
  fn borrow_mut(&self) -> Self::RefMut<'_>;

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
  fn with_ref<R>(&self, f: impl FnOnce(&T) -> R) -> R {
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
  fn with_ref_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
    let mut guard = self.borrow_mut();
    f(&mut *guard)
  }
}
