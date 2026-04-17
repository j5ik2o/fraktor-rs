//! Mutable state of [`OptimalSizeExploringResizer`](super::OptimalSizeExploringResizer).

use alloc::collections::BTreeMap;
use core::time::Duration;

use super::{lcg::Lcg, resize_record::ResizeRecord};

/// Mutable bookkeeping protected by the resizer's spin mutex.
///
/// Exposed at `pub(crate)` visibility so that the parent resizer type, its
/// tests, and the crate-internal routing machinery can mutate fields via
/// `SpinSyncMutex::lock`.
pub(crate) struct State<I> {
  /// Historical mean processing time per pool size.
  pub(crate) performance_log: BTreeMap<usize, Duration>,
  /// Snapshot of the previous sample.
  pub(crate) record:          ResizeRecord<I>,
  /// Seedable pseudo-random source used for explore / optimize branching.
  pub(crate) rng:             Lcg,
}
