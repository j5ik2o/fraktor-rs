//! Bounded actor reference resolution cache.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use crate::{
  actor::actor_path::{ActorPath, GuardianKind},
  serialization::ActorRefResolveCacheOutcome,
};

const DEFAULT_CAPACITY: usize = 1024;
const DEFAULT_EVICT_AGE_THRESHOLD: u64 = 600;
const TEMP_SEGMENT: &str = "temp";

/// Bounded cache for resolving canonical actor paths to actor references.
///
/// Pekko keeps this cache as a thread-local `LruBoundedCache[String, ActorRef]`.
/// fraktor keeps it as an owned mutable value so the provider that performs
/// resolution also owns cache updates.
#[derive(Clone, Debug)]
pub struct ActorRefResolveCache<V> {
  entries:             Vec<ActorRefResolveCacheEntry<V>>,
  capacity:            usize,
  evict_age_threshold: u64,
  epoch:               u64,
}

impl<V> ActorRefResolveCache<V>
where
  V: Clone,
{
  /// Creates a cache using explicit capacity and eviction age threshold.
  ///
  /// # Panics
  ///
  /// Panics when `capacity` is zero.
  #[must_use]
  pub fn with_limits(capacity: usize, evict_age_threshold: u64) -> Self {
    assert!(capacity > 0, "capacity must be larger than zero");
    Self { entries: Vec::with_capacity(capacity), capacity, evict_age_threshold, epoch: 0 }
  }

  /// Resolves `path`, returning whether the value came from cache or resolver.
  ///
  /// # Errors
  ///
  /// Returns the resolver error unchanged. Failed resolutions are not cached.
  pub fn resolve<E>(
    &mut self,
    path: &ActorPath,
    mut resolver: impl FnMut(&ActorPath) -> Result<V, E>,
  ) -> Result<ActorRefResolveCacheOutcome<V>, E> {
    let key = path.to_canonical_uri();
    self.advance_epoch();
    if let Some(index) = self.find_entry(&key) {
      self.entries[index].accessed_at = self.epoch;
      return Ok(ActorRefResolveCacheOutcome::Hit(self.entries[index].value.clone()));
    }

    let value = resolver(path)?;
    if is_cacheable_path(path) {
      self.insert(key, value.clone());
    }
    Ok(ActorRefResolveCacheOutcome::Miss(value))
  }

  const fn advance_epoch(&mut self) {
    self.epoch = self.epoch.saturating_add(1);
  }

  fn find_entry(&self, key: &str) -> Option<usize> {
    self.entries.iter().position(|entry| entry.key == key)
  }

  fn insert(&mut self, key: String, value: V) {
    if self.entries.len() == self.capacity {
      let index = self.evictable_index();
      drop(self.entries.remove(index));
    }
    self.entries.push(ActorRefResolveCacheEntry { key, value, accessed_at: self.epoch });
  }

  fn evictable_index(&self) -> usize {
    if let Some(index) = self.stale_entry_index() {
      return index;
    }
    self.least_recently_used_index()
  }

  fn stale_entry_index(&self) -> Option<usize> {
    // 複数 stale entry が存在する場合は最古 (accessed_at 最小) を選ぶ。 Vec の挿入順
    // (`position`) で選ぶと、 age threshold を辛うじて越えた直近アクセスの entry が、 さらに古い
    // 未アクセス entry より先に evict され LRU 意図を崩すため。
    self
      .entries
      .iter()
      .enumerate()
      .filter(|(_, entry)| self.epoch.saturating_sub(entry.accessed_at) >= self.evict_age_threshold)
      .min_by_key(|(_, entry)| entry.accessed_at)
      .map(|(index, _)| index)
  }

  fn least_recently_used_index(&self) -> usize {
    let mut selected_index = 0;
    let mut selected_accessed_at = self.entries[0].accessed_at;
    for (index, entry) in self.entries.iter().enumerate().skip(1) {
      if entry.accessed_at < selected_accessed_at {
        selected_index = index;
        selected_accessed_at = entry.accessed_at;
      }
    }
    selected_index
  }
}

impl<V> Default for ActorRefResolveCache<V>
where
  V: Clone,
{
  fn default() -> Self {
    Self::with_limits(DEFAULT_CAPACITY, DEFAULT_EVICT_AGE_THRESHOLD)
  }
}

#[derive(Clone, Debug)]
struct ActorRefResolveCacheEntry<V> {
  key:         String,
  value:       V,
  accessed_at: u64,
}

fn is_cacheable_path(path: &ActorPath) -> bool {
  // Pekko の `/user/temp/<name>` 形式の一時 actor は終了済み参照を再利用しないよう cache から外す。
  // `path.segments()` は guardian segment を先頭に保持しているため、 [guardian, temp, _] の 3
  // 要素で判定する。
  if path.guardian() != GuardianKind::User {
    return true;
  }
  !matches!(
    path.segments(),
    [guardian, second, _name] if guardian.as_str() == GuardianKind::User.segment() && second.as_str() == TEMP_SEGMENT
  )
}
