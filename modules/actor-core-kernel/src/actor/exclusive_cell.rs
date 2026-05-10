//! CAS-backed exclusive cell for actor hot-path shared wrappers.

#[cfg(test)]
mod tests;

use core::{cell::UnsafeCell, hint::spin_loop};

use portable_atomic::{AtomicBool, Ordering};

/// Shared interior-mutability primitive that grants access to one CAS winner at a time.
pub(crate) struct ExclusiveCell<T> {
  claimed: AtomicBool,
  value:   UnsafeCell<T>,
}

// SAFETY: `ExclusiveCell` は CAS claim 保持中にだけ参照を渡す。`T` が thread 間を
// 移動できるなら、cell 自体を thread 間で移動しても sound。
unsafe impl<T: Send> Send for ExclusiveCell<T> {}
// SAFETY: 共有参照経由のアクセスは CAS claim で直列化されるため `T: Sync` は不要。
// mutex 型の共有と同じく、排他アクセスだけを提供するので `T: Send` で足りる。
unsafe impl<T: Send> Sync for ExclusiveCell<T> {}

impl<T> ExclusiveCell<T> {
  /// Creates a new exclusive cell.
  #[must_use]
  pub(crate) const fn new(value: T) -> Self {
    Self { claimed: AtomicBool::new(false), value: UnsafeCell::new(value) }
  }

  /// Executes `f` with read access while holding the exclusive claim.
  pub(crate) fn with_read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    let _claim = self.claim();
    // SAFETY: `_claim` が drop されるまで排他的な CAS claim を保持するため、
    // 並行する mutable access は存在しない。read も同じ claim で直列化される。
    f(unsafe { &*self.value.get() })
  }

  /// Executes `f` with mutable access while holding the exclusive claim.
  pub(crate) fn with_write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
    let _claim = self.claim();
    // SAFETY: `_claim` が drop されるまで排他的な CAS claim を保持するため、
    // `f` の実行中に cell が渡す read/write 参照はこれだけ。
    f(unsafe { &mut *self.value.get() })
  }

  fn claim(&self) -> ExclusiveClaim<'_, T> {
    while self.claimed.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
      spin_loop();
    }
    ExclusiveClaim { cell: self }
  }
}

struct ExclusiveClaim<'a, T> {
  cell: &'a ExclusiveCell<T>,
}

impl<T> Drop for ExclusiveClaim<'_, T> {
  fn drop(&mut self) {
    self.cell.claimed.store(false, Ordering::Release);
  }
}
