//! UID 非依存の ActorPath 等価性ユーティリティ。

use core::hash::{BuildHasher, Hash, Hasher};

use hashbrown::hash_map::DefaultHashBuilder;

use super::ActorPath;

/// ActorPath を UID を無視して比較／ハッシュ化するヘルパ。
pub struct ActorPathComparator;

impl ActorPathComparator {
  /// UID を無視して等価性を判定する。
  #[must_use]
  pub fn eq(lhs: &ActorPath, rhs: &ActorPath) -> bool {
    lhs.parts() == rhs.parts() && lhs.segments() == rhs.segments()
  }

  /// UID を無視したハッシュ値を計算する。
  #[must_use]
  pub fn hash(path: &ActorPath) -> u64 {
    let mut hasher = DefaultHashBuilder::default().build_hasher();
    path.parts().hash(&mut hasher);
    path.segments().hash(&mut hasher);
    hasher.finish()
  }
}
