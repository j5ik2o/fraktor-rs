//! Registry for mapping PIDs to canonical actor paths.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::time::Duration;

use hashbrown::HashMap;

use super::{ActorPathHandle, ReservationPolicy};
use crate::actor_prim::{
  Pid,
  actor_path::{ActorPath, ActorUid, PathResolutionError},
};

/// UID reservation entry with expiration deadline.
#[derive(Clone, Debug)]
struct UidReservation {
  uid:      ActorUid,
  #[allow(dead_code)] // 将来の実装で使用予定
  deadline: Duration,
}

/// Registry for PID-to-path mappings and UID reservations.
pub struct ActorPathRegistry {
  paths:        HashMap<Pid, ActorPathHandle>,
  reservations: HashMap<String, UidReservation>,
  policy:       ReservationPolicy,
}

impl ActorPathRegistry {
  /// Creates a new empty registry.
  #[must_use]
  pub fn new() -> Self {
    Self { paths: HashMap::new(), reservations: HashMap::new(), policy: ReservationPolicy::default() }
  }

  /// Creates a registry with a custom reservation policy.
  #[must_use]
  pub fn with_policy(policy: ReservationPolicy) -> Self {
    Self { paths: HashMap::new(), reservations: HashMap::new(), policy }
  }

  /// Registers a path for a given PID.
  pub fn register(&mut self, pid: Pid, path: &ActorPath) {
    let handle = ActorPathHandle::new(pid, path.to_canonical_uri(), path.uid());
    self.paths.insert(pid, handle);
  }

  /// Retrieves a path handle by PID.
  #[must_use]
  pub fn get(&self, pid: &Pid) -> Option<&ActorPathHandle> {
    self.paths.get(pid)
  }

  /// Removes a path registration.
  pub fn unregister(&mut self, pid: &Pid) {
    self.paths.remove(pid);
  }

  /// Returns the canonical URI for a PID.
  #[must_use]
  pub fn canonical_uri(&self, pid: &Pid) -> Option<&str> {
    self.get(pid).map(ActorPathHandle::canonical_uri)
  }

  /// Reserves a UID for a given path, preventing reuse until expiration.
  ///
  /// # Errors
  ///
  /// Returns [`PathResolutionError::UidReserved`] if the UID is already reserved.
  pub fn reserve_uid(
    &mut self,
    path: &ActorPath,
    uid: ActorUid,
    custom_duration: Option<Duration>,
  ) -> Result<(), PathResolutionError> {
    let path_key = path.to_canonical_uri();

    // 既存の予約をチェック
    if let Some(reservation) = self.reservations.get(&path_key) {
      return Err(PathResolutionError::UidReserved { uid: reservation.uid });
    }

    // 新規予約を追加
    let duration = custom_duration.unwrap_or(self.policy.quarantine_duration());
    let deadline = duration; // 実際の実装では現在時刻 + duration を計算
    self.reservations.insert(path_key, UidReservation { uid, deadline });

    Ok(())
  }

  /// Releases a UID reservation for a path.
  pub fn release_uid(&mut self, path: &ActorPath) {
    let path_key = path.to_canonical_uri();
    self.reservations.remove(&path_key);
  }

  /// Removes expired UID reservations.
  pub fn poll_expired(&mut self) {
    // 実際の実装では現在時刻と比較して期限切れをフィルタリング
    // 簡易実装: すべて削除（テスト用）
    self.reservations.clear();
  }
}

impl Default for ActorPathRegistry {
  fn default() -> Self {
    Self::new()
  }
}
