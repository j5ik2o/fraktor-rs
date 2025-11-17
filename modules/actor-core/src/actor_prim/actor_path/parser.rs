//! Parser that converts canonical Fraktor URIs into actor paths.

#[cfg(test)]
mod tests;

use alloc::{
  string::{String, ToString},
  vec::Vec,
};

use fraktor_utils_core_rs::core::net::{UriError, UriParser};

use super::{ActorPath, ActorPathError, ActorPathParts, ActorPathScheme, ActorUid, GuardianKind, PathSegment};

/// Parses Pekko-compatible canonical URIs into [`ActorPath`].
pub struct ActorPathParser;

type ParsedAuthority<'a> = (&'a str, Option<(String, Option<u16>)>);

impl ActorPathParser {
  /// Restores an [`ActorPath`] from a canonical URI string.
  ///
  /// # Errors
  ///
  /// Returns [`ActorPathError`] when the given URI violates actor-path rules.
  pub fn parse(input: &str) -> Result<ActorPath, ActorPathError> {
    let uri = UriParser::parse(input).map_err(|_| ActorPathError::InvalidUri)?;
    let scheme = Self::parse_scheme(uri.scheme)?;
    let authority = uri.authority.ok_or(ActorPathError::MissingSystemName)?;
    let (system_name, host_info) = Self::split_authority(authority)?;

    let mut parts = ActorPathParts::local(system_name.to_string()).with_scheme(scheme);
    if let Some((host, port)) = host_info {
      parts = parts.with_authority_host(host);
      if let Some(port) = port {
        parts = parts.with_authority_port(port);
      }
    }

    // guardian は path 側に含まれているため、System guardian を指定する場合のみ上書きする。
    if ActorPathParser::path_starts_with_system(uri.path) {
      parts = parts.with_guardian(GuardianKind::System);
    }

    let segments = Self::parse_segments(uri.path)?;
    let uid = uri.fragment.and_then(|raw| raw.parse::<u64>().ok()).map(ActorUid::new);

    Ok(ActorPath::from_parts_and_segments(parts, segments, uid))
  }

  fn parse_scheme(raw: Option<&str>) -> Result<ActorPathScheme, ActorPathError> {
    match raw {
      | Some("fraktor") | None => Ok(ActorPathScheme::Fraktor),
      | Some("fraktor.tcp") => Ok(ActorPathScheme::FraktorTcp),
      | _ => Err(ActorPathError::UnsupportedScheme),
    }
  }

  fn split_authority(authority: &str) -> Result<ParsedAuthority<'_>, ActorPathError> {
    if authority.is_empty() {
      return Err(ActorPathError::MissingSystemName);
    }
    let mut parts = authority.splitn(2, '@');
    let system_name = parts.next().unwrap_or_default();
    if system_name.is_empty() {
      return Err(ActorPathError::MissingSystemName);
    }
    if let Some(authority_host) = parts.next() {
      let (host, port) = Self::parse_host_and_port(authority_host)?;
      Ok((system_name, Some((host, port))))
    } else {
      Ok((system_name, None))
    }
  }

  fn parse_host_and_port(authority_host: &str) -> Result<(String, Option<u16>), ActorPathError> {
    if authority_host.is_empty() {
      return Err(ActorPathError::InvalidAuthority);
    }

    if authority_host.starts_with('[') {
      let end = authority_host.find(']').ok_or(ActorPathError::InvalidAuthority)?;
      let host = &authority_host[..=end];
      UriParser::validate_hostname(host).map_err(|_| ActorPathError::InvalidAuthority)?;
      let port = if end + 1 < authority_host.len() {
        let remainder = authority_host[end + 1..].strip_prefix(':').ok_or(ActorPathError::InvalidAuthority)?;
        Some(Self::parse_port(remainder)?)
      } else {
        None
      };
      return Ok((host.to_string(), port));
    }

    let split_index = authority_host.rfind(':');
    if let Some(idx) = split_index {
      let host = &authority_host[..idx];
      let port = &authority_host[idx + 1..];
      if host.is_empty() {
        return Err(ActorPathError::InvalidAuthority);
      }
      UriParser::validate_hostname(host).map_err(|_| ActorPathError::InvalidAuthority)?;
      let parsed_port = Self::parse_port(port)?;
      Ok((host.to_string(), Some(parsed_port)))
    } else {
      UriParser::validate_hostname(authority_host).map_err(|_| ActorPathError::InvalidAuthority)?;
      Ok((authority_host.to_string(), None))
    }
  }

  fn parse_port(port: &str) -> Result<u16, ActorPathError> {
    port.parse::<u16>().map_err(|_| ActorPathError::InvalidAuthority)
  }

  fn parse_segments(path: &str) -> Result<Vec<PathSegment>, ActorPathError> {
    if path.is_empty() {
      return Ok(Vec::new());
    }
    let mut segments = Vec::new();
    for segment in path.split('/').filter(|seg| !seg.is_empty()) {
      segments.push(PathSegment::new(segment.to_string())?);
    }
    Ok(segments)
  }

  fn path_starts_with_system(path: &str) -> bool {
    path.starts_with("/system")
  }
}

impl From<UriError> for ActorPathError {
  fn from(_: UriError) -> Self {
    ActorPathError::InvalidUri
  }
}
