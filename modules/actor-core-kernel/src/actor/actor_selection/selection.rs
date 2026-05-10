//! Public handle for classic actor selection operations.

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::{
  fmt::{Debug, Formatter, Result as FmtResult},
  time::Duration,
};

use super::{ActorSelectionError, ActorSelectionResolver};
use crate::{
  actor::{
    actor_path::{
      ActorPath, ActorPathError, ActorPathParser, ActorPathParts, ActorPathScheme, GuardianKind, PathResolutionError,
      PathSegment,
    },
    actor_ref::ActorRef,
    actor_ref_provider::ActorRefResolveError,
    messaging::{AnyMessage, AskResponse, Identify},
  },
  system::{
    ActorSystem,
    state::{AuthorityState, SystemStateShared},
  },
};

/// Classic actor selection handle.
pub struct ActorSelection {
  system:    SystemStateShared,
  base_path: ActorPath,
  selection: String,
}

impl ActorSelection {
  pub(crate) const fn new(system: SystemStateShared, base_path: ActorPath, selection: String) -> Self {
    Self { system, base_path, selection }
  }

  pub(crate) fn from_path(system: SystemStateShared, path: &ActorPath) -> Self {
    let base_path = Self::canonicalize_base_path(&system, path);
    Self::new(system, base_path, String::new())
  }

  /// Sends a fire-and-forget message to the selected actor.
  ///
  /// # Errors
  ///
  /// Returns an error if the path cannot be resolved or delivery is rejected.
  pub fn tell(&self, message: AnyMessage, sender: Option<ActorRef>) -> Result<(), ActorSelectionError> {
    let message = match sender {
      | Some(sender) => message.with_sender(sender),
      | None => message,
    };
    self.deliver(message)
  }

  /// Forwards a message while preserving the original sender.
  ///
  /// # Errors
  ///
  /// Returns an error if the path cannot be resolved or delivery is rejected.
  pub fn forward(&self, message: AnyMessage, sender: &ActorRef) -> Result<(), ActorSelectionError> {
    self.deliver(message.with_sender(sender.clone()))
  }

  /// Resolves the selected actor and sends an `Identify` request via ask.
  ///
  /// # Errors
  ///
  /// Returns an error if the path cannot be resolved before the ask starts.
  pub fn resolve_one(&self, timeout: Duration) -> Result<AskResponse, ActorSelectionError> {
    let path = self.resolve_target_path()?;
    self.ensure_authority_state(&path, None)?;
    let mut actor_ref = self.resolve_actor_ref(path)?;
    Ok(actor_ref.ask_with_timeout(self.build_identify_envelope(), timeout))
  }

  /// Wraps the `Identify` payload into an `AnyMessage` that carries the
  /// `NotInfluenceReceiveTimeout` marker, so the receiving actor does not
  /// reset its receive timeout when answering the identity request
  /// (Pekko `Actor.scala:81`). Exposed for `pub(crate)` unit tests.
  #[must_use]
  pub(crate) fn build_identify_envelope(&self) -> AnyMessage {
    AnyMessage::not_influence(Identify::new(AnyMessage::new(self.selection.clone())))
  }

  /// Returns a canonical string representation suitable for later reconstruction.
  ///
  /// # Errors
  ///
  /// Returns an error when the selection expression itself is invalid.
  pub fn to_serialization_format(&self) -> Result<String, ActorSelectionError> {
    Ok(self.resolve_target_path()?.to_canonical_uri())
  }

  fn deliver(&self, message: AnyMessage) -> Result<(), ActorSelectionError> {
    let path = self.resolve_target_path()?;
    self.ensure_authority_state(&path, Some(&message))?;
    let mut actor_ref = self.resolve_actor_ref(path)?;
    actor_ref.try_tell(message).map_err(ActorSelectionError::from)
  }

  fn resolve_actor_ref(&self, path: ActorPath) -> Result<ActorRef, ActorSelectionError> {
    let system = ActorSystem::from_system_state(self.system.clone());
    if let Some(pid) = system.pid_by_path(&path)
      && let Some(actor_ref) = system.actor_ref_by_pid(pid)
    {
      return Ok(actor_ref);
    }
    system.resolve_actor_ref(path).map_err(ActorSelectionError::from)
  }

  fn resolve_target_path(&self) -> Result<ActorPath, ActorSelectionError> {
    if self.selection.is_empty() {
      return Ok(self.base_path.clone());
    }
    if self.selection.contains("://") {
      return ActorPathParser::parse(&self.selection).map_err(ActorSelectionError::from);
    }
    if self.selection.starts_with('/') {
      return Self::resolve_absolute(&self.base_path, &self.selection).map_err(ActorSelectionError::from);
    }
    ActorSelectionResolver::resolve_relative(&self.base_path, &self.selection).map_err(ActorSelectionError::from)
  }

  fn ensure_authority_state(&self, path: &ActorPath, message: Option<&AnyMessage>) -> Result<(), ActorSelectionError> {
    let Some(authority) = path.parts().authority_endpoint() else {
      return Ok(());
    };
    match self.system.remote_authority_state(&authority) {
      | AuthorityState::Connected => Ok(()),
      | AuthorityState::Unresolved => {
        if let Some(message) = message {
          self
            .system
            .remote_authority_defer(authority, message.clone())
            .map_err(|_| ActorSelectionError::from(PathResolutionError::AuthorityQuarantined))?;
        }
        Err(ActorSelectionError::from(PathResolutionError::AuthorityUnresolved))
      },
      | AuthorityState::Quarantine { .. } => Err(ActorSelectionError::from(PathResolutionError::AuthorityQuarantined)),
    }
  }

  fn resolve_absolute(base: &ActorPath, selection: &str) -> Result<ActorPath, ActorPathError> {
    let trimmed = selection.trim_start_matches('/');
    let raw_segments: Vec<&str> = trimmed.split('/').filter(|segment| !segment.is_empty()).collect();
    let guardian = match raw_segments.first().copied() {
      | Some("system") => GuardianKind::System,
      | Some("user") => GuardianKind::User,
      | _ => base.guardian(),
    };
    let parts = Self::parts_with_guardian(base.parts().clone(), guardian);
    let segments =
      raw_segments.into_iter().map(|segment| PathSegment::new(segment.to_string())).collect::<Result<Vec<_>, _>>()?;
    Ok(ActorPath::from_parts_and_segments(parts, segments, None))
  }

  const fn parts_with_guardian(parts: ActorPathParts, guardian: GuardianKind) -> ActorPathParts {
    parts.with_guardian(guardian)
  }

  fn canonicalize_base_path(system: &SystemStateShared, path: &ActorPath) -> ActorPath {
    if path.parts().authority_endpoint().is_some() || path.parts().system() != "cellactor" {
      return path.clone();
    }
    let mut parts = ActorPathParts::local(system.system_name()).with_guardian(path.guardian());
    if let Some((host, Some(port))) = system.canonical_authority_components() {
      parts = parts.with_scheme(ActorPathScheme::FraktorTcp).with_authority_host(host).with_authority_port(port);
    }
    ActorPath::from_parts_and_segments(parts, path.segments().to_vec(), path.uid())
  }
}

impl Debug for ActorSelection {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("ActorSelection").field("base_path", &self.base_path).field("selection", &self.selection).finish()
  }
}

impl From<ActorRefResolveError> for ActorSelectionError {
  fn from(error: ActorRefResolveError) -> Self {
    Self::Resolve(error)
  }
}
