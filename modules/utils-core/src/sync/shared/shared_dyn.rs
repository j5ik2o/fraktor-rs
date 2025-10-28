use super::shared_trait::Shared;

/// Extensions for shared handles that can be converted into trait objects.
pub trait SharedDyn<T: ?Sized>: Shared<T> {
  /// Shared wrapper yielded after converting to a new dynamically sized view.
  type Dyn<U: ?Sized + 'static>: Shared<U>;

  /// Converts the shared handle into another dynamically sized representation.
  fn into_dyn<U: ?Sized + 'static, F>(self, cast: F) -> Self::Dyn<U>
  where
    F: FnOnce(&T) -> &U;
}
