//! Name registry implementation.

use alloc::{borrow::ToOwned, collections::BTreeMap, format, string::String};

use crate::pid::Pid;

/// Tracks actor names within a supervisor scope.
#[derive(Debug, Default)]
pub struct NameRegistry {
  entries: BTreeMap<String, Pid>,
  counter: u64,
}

impl NameRegistry {
  /// Creates an empty registry.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: BTreeMap::new(), counter: 0 }
  }

  /// Attempts to bind the provided name to the specified PID.
  pub fn register<S>(&mut self, name: S, pid: Pid) -> Result<(), ()>
  where
    S: AsRef<str>, {
    let key = name.as_ref();
    if self.entries.contains_key(key) {
      return Err(());
    }
    self.entries.insert(key.to_owned(), pid);
    Ok(())
  }

  /// Releases a name from the registry.
  pub fn unregister(&mut self, name: &str) -> Option<Pid> {
    self.entries.remove(name)
  }

  /// Resolves a name back to a PID if present.
  #[must_use]
  pub fn lookup(&self, name: &str) -> Option<Pid> {
    self.entries.get(name).copied()
  }

  /// Allocates a unique anonymous name of the form `anon-<counter>`.
  pub fn allocate_anonymous(&mut self) -> String {
    loop {
      let candidate = format!("anon-{}", self.counter);
      self.counter = self.counter.wrapping_add(1);
      if !self.entries.contains_key(&candidate) {
        break candidate;
      }
    }
  }
}
