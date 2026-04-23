use super::shared_trait::Shared;

/// Extensions for shared handles that can be converted into trait objects.
///
/// Only exercised via tests that call the trait method through fully-qualified
/// syntax (`<ArcShared<T> as SharedDyn<T>>::into_dyn`). Production callers use
/// the inherent [`ArcShared::into_dyn`](super::super::arc_shared::ArcShared::into_dyn)
/// directly, which is why the trait must stay `#[allow(dead_code)]`.
#[allow(dead_code)]
pub(crate) trait SharedDyn<T: ?Sized>: Shared<T> {
  /// Shared wrapper yielded after converting to a new dynamically sized view.
  type Dyn<U: ?Sized + 'static>: Shared<U>;

  /// Converts the shared handle into another dynamically sized representation.
  fn into_dyn<U: ?Sized + 'static, F>(self, cast: F) -> Self::Dyn<U>
  where
    F: FnOnce(&T) -> &U;
}
