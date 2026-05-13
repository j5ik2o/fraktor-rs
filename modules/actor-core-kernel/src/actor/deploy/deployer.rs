use alloc::{collections::BTreeMap, string::String};

use super::Deploy;

#[cfg(test)]
#[path = "deployer_test.rs"]
mod tests;

/// Registry of classic deployment descriptors keyed by logical path.
#[derive(Clone, Debug, Default)]
pub struct Deployer {
  entries: BTreeMap<String, Deploy>,
}

impl Deployer {
  /// Creates an empty deployer registry.
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Registers or updates a deployment descriptor.
  pub fn register(&mut self, path: impl Into<String>, deploy: Deploy) {
    self.entries.insert(path.into(), deploy);
  }

  /// Returns the registered deployment descriptor for the path.
  #[must_use]
  pub fn deploy_for(&self, path: &str) -> Option<&Deploy> {
    self.entries.get(path)
  }
}
