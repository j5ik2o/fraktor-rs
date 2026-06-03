//! Canonical address-less registry key for mediator path commands.

use alloc::{format, string::String, vec::Vec};
use core::fmt::{Display, Formatter, Result as FmtResult};

use fraktor_actor_core_kernel_rs::actor::{
  actor_path::{ActorPath, ActorPathParser, GuardianKind},
  actor_selection::ActorSelectionResolver,
};

use super::PubSubError;

/// Address-less actor path key used by mediator path registry entries.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MediatorPathKey(String);

impl MediatorPathKey {
  /// Parses an actor path URI or selection and keeps only the relative path representation.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidPath`] when the raw path is empty or actor-core rejects it.
  pub fn parse(raw: &str) -> Result<Self, PubSubError> {
    if raw.is_empty() {
      return Err(PubSubError::InvalidPath { reason: String::from("path must not be empty") });
    }
    let path = Self::resolve_path(raw)?;
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

  fn resolve_path(raw: &str) -> Result<ActorPath, PubSubError> {
    if raw.contains("://") {
      return ActorPathParser::parse(raw)
        .map_err(|error| PubSubError::InvalidPath { reason: format!("invalid actor path: {error:?}") });
    }
    if raw.starts_with('/') {
      return Self::resolve_absolute(raw);
    }
    ActorSelectionResolver::resolve_relative(&Self::relative_base()?, raw)
      .map_err(|error| PubSubError::InvalidPath { reason: format!("invalid actor selection: {error:?}") })
  }

  fn resolve_absolute(selection: &str) -> Result<ActorPath, PubSubError> {
    let trimmed = selection.trim_start_matches('/');
    let raw_segments = trimmed.split('/').filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
    let (guardian, skip_guardian) = match raw_segments.first().copied() {
      | Some("system") => (GuardianKind::System, 1),
      | Some("user") => (GuardianKind::User, 1),
      | Some(_) => {
        return Err(PubSubError::InvalidPath {
          reason: format!("absolute actor selection must start with /user or /system: {selection}"),
        });
      },
      | None => (GuardianKind::User, 0),
    };
    let mut path = ActorPath::root_with_guardian(guardian);
    for segment in raw_segments.into_iter().skip(skip_guardian) {
      path = path
        .try_child(segment)
        .map_err(|error| PubSubError::InvalidPath { reason: format!("invalid actor selection: {error:?}") })?;
    }
    Ok(path)
  }

  fn relative_base() -> Result<ActorPath, PubSubError> {
    ActorPath::root()
      .try_child("mediator")
      .map_err(|error| PubSubError::InvalidPath { reason: format!("invalid mediator base path: {error:?}") })
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
