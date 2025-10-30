//! Actor system placeholder implementation.

use crate::props::Props;

/// Minimal actor system placeholder. 実際の実装は後続タスクで追加する。
pub struct ActorSystem {
  _unused: (),
}

impl ActorSystem {
  /// Creates a new actor system placeholder.
  #[must_use]
  pub const fn new_empty() -> Self {
    Self { _unused: () }
  }

  /// Spawns a new actor using the supplied props.
  pub fn spawn(&self, _props: Props) {
    // 実装は後続フェーズで追加する。
  }
}
