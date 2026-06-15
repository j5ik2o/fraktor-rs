//! Reachability matrix for observer-subject membership evidence.

#[cfg(test)]
#[path = "reachability_matrix_test.rs"]
mod tests;

use alloc::{collections::BTreeMap, vec::Vec};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{IndirectConnectionEvidence, ReachabilityRecord, ReachabilitySnapshot, ReachabilityStatus};

/// Observer-subject reachability matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachabilityMatrix {
  records:           BTreeMap<(UniqueAddress, UniqueAddress), ReachabilityRecord>,
  observer_versions: BTreeMap<UniqueAddress, u64>,
}

impl ReachabilityMatrix {
  /// Creates an empty reachability matrix.
  #[must_use]
  pub const fn new() -> Self {
    Self { records: BTreeMap::new(), observer_versions: BTreeMap::new() }
  }

  /// Records that `observer` currently sees `subject` as unreachable.
  pub fn unreachable(&mut self, observer: UniqueAddress, subject: UniqueAddress) {
    self.update(observer, subject, ReachabilityStatus::Unreachable);
  }

  /// Records that `observer` currently sees `subject` as reachable.
  pub fn reachable(&mut self, observer: UniqueAddress, subject: UniqueAddress) {
    self.update(observer, subject, ReachabilityStatus::Reachable);
  }

  /// Records that `observer` currently sees `subject` as terminated.
  pub fn terminated(&mut self, observer: UniqueAddress, subject: UniqueAddress) {
    self.update(observer, subject, ReachabilityStatus::Terminated);
  }

  /// Clears all reachability records for a subject.
  pub fn clear_subject(&mut self, subject: &UniqueAddress) {
    let observers = self
      .records
      .keys()
      .filter(|(_, record_subject)| record_subject == subject)
      .map(|(observer, _)| observer.clone())
      .collect::<Vec<_>>();
    for observer in observers {
      self.records.remove(&(observer.clone(), subject.clone()));
      self.bump_observer_version(observer);
    }
  }

  /// Clears all reachability records reported by an observer.
  pub fn clear_observer(&mut self, observer: &UniqueAddress) {
    let subjects = self
      .records
      .keys()
      .filter(|(record_observer, _)| record_observer == observer)
      .map(|(_, subject)| subject.clone())
      .collect::<Vec<_>>();
    if subjects.is_empty() {
      return;
    }
    for subject in subjects {
      self.records.remove(&(observer.clone(), subject));
    }
    self.bump_observer_version(observer.clone());
  }

  /// Returns the aggregate status for a subject across all observers.
  #[must_use]
  pub fn aggregate_status(&self, subject: &UniqueAddress) -> ReachabilityStatus {
    let mut aggregate = ReachabilityStatus::Reachable;
    for record in self.records.values().filter(|record| &record.subject == subject) {
      match record.status {
        | ReachabilityStatus::Terminated => return ReachabilityStatus::Terminated,
        | ReachabilityStatus::Unreachable => aggregate = ReachabilityStatus::Unreachable,
        | ReachabilityStatus::Reachable => {},
      }
    }
    aggregate
  }

  /// Returns an immutable snapshot of matrix records and observer row versions.
  #[must_use]
  pub fn snapshot(&self) -> ReachabilitySnapshot {
    ReachabilitySnapshot::new(self.records.values().cloned().collect::<Vec<_>>(), self.observer_versions.clone())
  }

  /// Returns indirect connectivity evidence for a partially reachable subject.
  #[must_use]
  pub fn indirect_evidence_for(&self, subject: &UniqueAddress) -> Option<IndirectConnectionEvidence> {
    let mut direct_observations = Vec::new();
    let mut indirect_observations = Vec::new();

    for (observer, version) in self.observer_versions.iter() {
      let key = (observer.clone(), subject.clone());
      if let Some(record) = self.records.get(&key) {
        if record.status != ReachabilityStatus::Reachable {
          direct_observations.push(record.clone());
        }
      } else {
        indirect_observations.push(ReachabilityRecord {
          observer: observer.clone(),
          subject:  subject.clone(),
          status:   ReachabilityStatus::Reachable,
          version:  *version,
        });
      }
    }

    if direct_observations.is_empty() || indirect_observations.is_empty() {
      return None;
    }

    let mut observer_aggregate_statuses = Vec::new();
    for observation in direct_observations.iter().chain(indirect_observations.iter()) {
      let observer = observation.observer.clone();
      let version = self.observer_versions.get(&observer).copied().unwrap_or(0);
      observer_aggregate_statuses.push(ReachabilityRecord {
        observer: observer.clone(),
        subject: observer.clone(),
        status: self.aggregate_status(&observer),
        version,
      });
    }

    Some(IndirectConnectionEvidence {
      subject: subject.clone(),
      direct_observations,
      indirect_observations,
      observer_aggregate_statuses,
    })
  }

  fn update(&mut self, observer: UniqueAddress, subject: UniqueAddress, status: ReachabilityStatus) {
    let key = (observer.clone(), subject.clone());

    if let Some(record) = self.records.get(&key) {
      if record.status == ReachabilityStatus::Terminated || record.status == status {
        return;
      }
    } else if status == ReachabilityStatus::Reachable {
      self.bump_observer_version(observer);
      return;
    }

    let version = self.bump_observer_version(observer.clone());
    if status == ReachabilityStatus::Reachable {
      self.records.remove(&key);
    } else {
      self.records.insert(key, ReachabilityRecord { observer, subject, status, version });
    }
  }

  fn bump_observer_version(&mut self, observer: UniqueAddress) -> u64 {
    let version = self.observer_versions.get(&observer).copied().unwrap_or(0) + 1;
    self.observer_versions.insert(observer, version);
    version
  }
}

impl Default for ReachabilityMatrix {
  fn default() -> Self {
    Self::new()
  }
}
