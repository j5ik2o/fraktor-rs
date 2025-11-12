//! Registry for mapping PIDs to canonical actor paths.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::time::Duration;

use hashbrown::HashMap;

use super::{ActorPathHandle, ReservationPolicy};
use crate::actor_prim::{
  Pid,
  actor_path::{ActorPath, ActorPathComparator, ActorUid, PathResolutionError},
};

/// UID reservation entry with expiration deadline.
#[derive(Clone, Debug)]
struct UidReservation {
  uid:      ActorUid,
  deadline: Option<u64>,
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

  /// Applies a new reservation policy.
  pub const fn set_policy(&mut self, policy: ReservationPolicy) {
    self.policy = policy;
  }

  /// Registers a path for a given PID.
  pub fn register(&mut self, pid: Pid, path: &ActorPath) {
    let handle = ActorPathHandle::new(pid, path.to_canonical_uri(), path.uid(), ActorPathComparator::hash(path));
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
    now_secs: u64,
    custom_duration: Option<Duration>,
  ) -> Result<(), PathResolutionError> {
    let path_key = Self::canonical_key(path);

    // 既存の予約をチェック
    if let Some(reservation) = self.reservations.get(&path_key) {
      return Err(PathResolutionError::UidReserved { uid: reservation.uid });
    }

    // 新規予約を追加
    let duration = custom_duration.unwrap_or(self.policy.quarantine_duration());
    let deadline = duration.as_secs().checked_add(now_secs).map(|instant| instant);
    self.reservations.insert(path_key, UidReservation { uid, deadline });

    Ok(())
  }

  /// Releases a UID reservation for a path.
  pub fn release_uid(&mut self, path: &ActorPath) {
    let path_key = Self::canonical_key(path);
    self.reservations.remove(&path_key);
  }

  /// Removes expired UID reservations.
  pub fn poll_expired(&mut self, now_secs: u64) {
    self.reservations.retain(|_, reservation| match reservation.deadline {
      | Some(deadline) => deadline > now_secs,
      | None => true,
    });
  }

  fn canonical_key(path: &ActorPath) -> String {
    let canonical = path.to_canonical_uri();
    if let Some(idx) = canonical.find('#') { String::from(&canonical[..idx]) } else { canonical }
  }
}

impl Default for ActorPathRegistry {
  fn default() -> Self {
    Self::new()
  }
}
