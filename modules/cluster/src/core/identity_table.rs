//! Table for resolving PIDs and determining authority status.

use alloc::{
  collections::BTreeMap,
  format,
  string::{String, ToString},
  vec::Vec,
};

use fraktor_actor_rs::core::actor_prim::actor_path::{ActorPathError, ActorPathParser};

use crate::core::{
  identity_event::IdentityEvent, membership_delta::MembershipDelta, membership_table::MembershipTable,
  node_status::NodeStatus, resolve_error::ResolveError, resolve_result::ResolveResult,
};

#[cfg(test)]
mod tests;

/// Resolves PIDs based on membership state.
pub struct IdentityTable {
  membership:  MembershipTable,
  quarantined: BTreeMap<String, String>,
  events:      Vec<IdentityEvent>,
}

impl IdentityTable {
  /// Creates a new identity table.
  #[must_use]
  pub const fn new(membership: MembershipTable) -> Self {
    Self { membership, quarantined: BTreeMap::new(), events: Vec::new() }
  }

  /// Registers a quarantine for an authority.
  pub fn quarantine(&mut self, authority: String, reason: String) {
    self.quarantined.insert(authority, reason);
  }

  /// Clears the quarantine for an authority.
  pub fn clear_quarantine(&mut self, authority: &str) {
    self.quarantined.remove(authority);
  }

  /// Applies a membership delta.
  pub fn apply_membership_delta(&mut self, delta: MembershipDelta) {
    self.membership.apply_delta(delta);
  }

  /// Resolves a PID from authority and path.
  ///
  /// # Errors
  ///
  /// Returns `ResolveError::InvalidFormat` if the authority or path format is invalid.
  pub fn resolve(&mut self, authority: &str, path: &str) -> Result<ResolveResult, ResolveError> {
    let version = self.membership.version();

    let canonical = match Self::build_canonical_uri(authority, path) {
      | Ok(uri) => uri,
      | Err(err) => {
        let reason = match &err {
          | ResolveError::InvalidFormat { reason } => reason.clone(),
        };
        self.events.push(IdentityEvent::InvalidFormat { reason });
        return Err(err);
      },
    };

    let actor_path = match ActorPathParser::parse(&canonical) {
      | Ok(path) => path,
      | Err(err) => {
        let reason = format_actor_path_error(&err);
        self.events.push(IdentityEvent::InvalidFormat { reason: reason.clone() });
        return Err(ResolveError::InvalidFormat { reason });
      },
    };

    if let Some(reason) = self.quarantined.get(authority) {
      let reason = reason.clone();
      self.events.push(IdentityEvent::Quarantined {
        authority: authority.to_string(),
        reason: reason.clone(),
        version,
      });
      return Ok(ResolveResult::Quarantine { authority: authority.to_string(), reason, version });
    }

    let Some(record) = self.membership.record(authority) else {
      self.events.push(IdentityEvent::UnknownAuthority { authority: authority.to_string(), version });
      return Ok(ResolveResult::Unreachable { authority: authority.to_string(), version });
    };

    match record.status {
      | NodeStatus::Removed | NodeStatus::Unreachable => {
        self.events.push(IdentityEvent::UnknownAuthority { authority: authority.to_string(), version });
        return Ok(ResolveResult::Unreachable { authority: authority.to_string(), version });
      },
      | _ => {},
    }

    self.events.push(IdentityEvent::ResolvedLatest { authority: authority.to_string(), version });

    Ok(ResolveResult::Ready { actor_path, version })
  }

  /// Drains accumulated events.
  pub fn drain_events(&mut self) -> Vec<IdentityEvent> {
    core::mem::take(&mut self.events)
  }

  fn build_canonical_uri(authority: &str, path: &str) -> Result<String, ResolveError> {
    if authority.is_empty() {
      return Err(ResolveError::InvalidFormat { reason: "authority is empty".to_string() });
    }
    if path.is_empty() {
      return Err(ResolveError::InvalidFormat { reason: "path is empty".to_string() });
    }

    let segments = path.split('/').filter(|s| !s.is_empty());
    if segments.clone().any(|s| s.contains(' ')) {
      return Err(ResolveError::InvalidFormat { reason: "path contains whitespace".to_string() });
    }

    // Validate host:port
    let mut authority_parts = authority.splitn(2, ':');
    let host = authority_parts.next().unwrap_or("");
    let port = authority_parts
      .next()
      .ok_or_else(|| ResolveError::InvalidFormat { reason: "authority missing port".to_string() })?
      .parse::<u16>()
      .map_err(|_| ResolveError::InvalidFormat { reason: "authority port invalid".to_string() })?;

    if host.is_empty() {
      return Err(ResolveError::InvalidFormat { reason: "authority missing host".to_string() });
    }

    let path_string: String = segments.clone().collect::<Vec<_>>().join("/");
    Ok(format!("fraktor.tcp://cellactor@{}:{}/{}", host, port, path_string))
  }
}

fn format_actor_path_error(err: &ActorPathError) -> String {
  match err {
    | ActorPathError::MissingSystemName => "missing system name".to_string(),
    | ActorPathError::InvalidAuthority => "invalid authority".to_string(),
    | ActorPathError::UnsupportedScheme => "unsupported scheme".to_string(),
    | ActorPathError::InvalidPercentEncoding => "invalid segment".to_string(),
    | ActorPathError::InvalidUri => "invalid uri".to_string(),
    | ActorPathError::EmptySegment => "empty segment".to_string(),
    | ActorPathError::ReservedSegment => "reserved segment".to_string(),
    | ActorPathError::InvalidSegmentChar { .. } => "invalid segment".to_string(),
    | ActorPathError::RelativeEscape => "relative escape".to_string(),
  }
}
