//! Scoped registry for mapping actor names to PIDs.

mod error;

use alloc::{collections::BTreeMap, format, string::String};

pub use error::NameRegistryError;

use crate::pid::Pid;

const ANON_PREFIX: &str = "anon-";

/// Tracks actor names scoped to a parent context.
pub struct NameRegistry {
  entries: BTreeMap<String, Pid>,
}

impl NameRegistry {
  /// Creates an empty name registry.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: BTreeMap::new() }
  }

  /// Registers a PID with an optional preferred name.
  pub fn register(&mut self, pid: Pid, preferred: Option<&str>) -> Result<String, NameRegistryError> {
    match preferred {
      | Some(name) => self.insert_named(pid, name),
      | None => self.insert_anonymous(pid),
    }
  }

  /// Removes a name from the registry, returning its PID when present.
  pub fn release(&mut self, name: &str) -> Option<Pid> {
    self.entries.remove(name)
  }

  /// Looks up a PID by name.
  #[must_use]
  pub fn lookup(&self, name: &str) -> Option<&Pid> {
    self.entries.get(name)
  }

  /// Returns `true` when the registry already contains the provided name.
  #[must_use]
  pub fn contains(&self, name: &str) -> bool {
    self.entries.contains_key(name)
  }

  fn insert_named(&mut self, pid: Pid, name: &str) -> Result<String, NameRegistryError> {
    if name.is_empty() {
      return Err(NameRegistryError::InvalidName);
    }

    if self.entries.contains_key(name) {
      return Err(NameRegistryError::DuplicateName(name.into()));
    }

    self.entries.insert(name.into(), pid);
    Ok(name.into())
  }

  fn insert_anonymous(&mut self, pid: Pid) -> Result<String, NameRegistryError> {
    let mut candidate = format!("{ANON_PREFIX}{}", pid.value());
    let mut counter: u32 = 1;

    while self.entries.contains_key(&candidate) {
      candidate = format!("{ANON_PREFIX}{}-{}", pid.value(), counter);
      counter = counter.saturating_add(1);
    }

    self.entries.insert(candidate.clone(), pid);
    Ok(candidate)
  }
}
