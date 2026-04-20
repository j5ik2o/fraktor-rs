//! Failure state tag attached to an [`ActorCell`] while fault handling is in
//! progress.
//!
//! Translated from Pekko `FaultHandling.scala`:
//! ```scala
//! private sealed trait FailedInfo
//! private case object NoFailedInfo extends FailedInfo
//! private case class  FailedRef(perpetrator: ActorRef) extends FailedInfo
//! private case object FailedFatally extends FailedInfo
//! ```
//!
//! The three states map directly to Pekko:
//! * [`FailedInfo::NoFailedInfo`] — the cell has never failed or was cleared
//!   by `clearFailed()`.
//! * [`FailedInfo::FailedRef`] — a child failure is being processed and the
//!   recorded [`Pid`] identifies the failing child (Pekko's `perpetrator`).
//! * [`FailedInfo::FailedFatally`] — the cell suffered a fatal failure (e.g.
//!   `Kill`) and cannot be restarted until explicitly cleared.
//!
//! [`ActorCell`]: crate::core::kernel::actor::ActorCell

use crate::core::kernel::actor::Pid;

/// Failure state tag stored inside `ActorCellState`.
///
/// See the module-level doc-comment for the full Pekko mapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FailedInfo {
  /// No failure is in flight. Equivalent to Pekko's `NoFailedInfo`.
  NoFailedInfo,
  /// A child failure is being processed. The payload carries the failing
  /// child's [`Pid`] (Pekko's `perpetrator`).
  FailedRef(Pid),
  /// A fatal failure has been recorded; the cell cannot be restarted. Pekko
  /// parity: `FailedFatally`.
  FailedFatally,
}
