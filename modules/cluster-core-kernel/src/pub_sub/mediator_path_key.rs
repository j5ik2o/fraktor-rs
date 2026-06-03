//! Canonical address-less registry key for mediator path commands.

use alloc::{format, string::String};
use core::fmt::{Display, Formatter, Result as FmtResult};

use fraktor_actor_core_kernel_rs::actor::actor_path::ActorPathParser;

use super::PubSubError;

/// Address-less actor path key used by mediator path registry entries.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MediatorPathKey(String);

impl MediatorPathKey {
  /// Parses a canonical actor path URI and keeps only the relative path representation.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidPath`] when the raw path is empty or actor-core rejects it.
  pub fn parse(raw: &str) -> Result<Self, PubSubError> {
    if raw.is_empty() {
      return Err(PubSubError::InvalidPath { reason: String::from("path must not be empty") });
    }
    let path = ActorPathParser::parse(raw)
      .map_err(|error| PubSubError::InvalidPath { reason: format!("invalid actor path: {error:?}") })?;
    if path.segments().len() <= 1 {
      return Err(PubSubError::InvalidPath { reason: String::from("path must include a child segment") });
    }
    let relative = path.to_relative_string();
    Ok(Self(relative))
  }

  /// Returns the address-less registry key.
  #[must_use]
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl From<MediatorPathKey> for String {
  fn from(value: MediatorPathKey) -> Self {
    value.0
  }
}

impl From<&MediatorPathKey> for String {
  fn from(value: &MediatorPathKey) -> Self {
    value.0.clone()
  }
}

impl Display for MediatorPathKey {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.write_str(&self.0)
  }
}
