#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::core::{materialization::ActorMaterializer, snapshot::StreamSnapshot};

/// Diagnostic facade over a materializer's stream state.
///
/// Mirrors Apache Pekko's `pekko.stream.snapshot.MaterializerState`. The Scala
/// version exposes `streamSnapshots(mat: Materializer): Future[Seq[StreamSnapshot]]`
/// because Pekko's supervisor aggregates snapshots asynchronously via the
/// `GetChildrenSnapshots` ask protocol. fraktor-rs instead reads each
/// registered [`crate::core::impl::materialization::StreamShared`]
/// synchronously under its `SharedLock`, so the return type is a plain
/// [`Vec<StreamSnapshot>`].
///
/// The type intentionally has no fields: it only provides the namespace for
/// the `stream_snapshots` helper, matching Pekko's object-style API.
pub struct MaterializerState;

impl MaterializerState {
  /// Collects one [`StreamSnapshot`] per stream registered with the
  /// materializer.
  ///
  /// Returns an empty [`Vec`] when the materializer has not materialized any
  /// streams yet, or after [`ActorMaterializer::shutdown`] has cleared the
  /// registered streams.
  #[must_use]
  pub fn stream_snapshots(mat: &ActorMaterializer) -> Vec<StreamSnapshot> {
    mat.streams().iter().map(|stream| stream.snapshot()).collect()
  }
}
