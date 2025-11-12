//! RFC2396-compliant URI parser.

use alloc::vec::Vec;
use core::fmt;

/// Parsed URI components according to RFC2396.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriParts<'a> {
  /// URI scheme (e.g., "pekko", "pekko.tcp").
  pub scheme:    Option<&'a str>,
  /// Authority component (host:port).
  pub authority: Option<&'a str>,
  /// Path component.
  pub path:      &'a str,
  /// Query component.
  pub query:     Option<&'a str>,
  /// Fragment component.
  pub fragment:  Option<&'a str>,
}

/// Errors that may occur during URI parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriError {
  /// Invalid scheme format.
  InvalidScheme,
  /// Invalid authority format.
  InvalidAuthority,
  /// Invalid path format.
  InvalidPath,
  /// Invalid query format.
  InvalidQuery,
  /// Invalid fragment format.
  InvalidFragment,
  /// Invalid percent encoding.
  InvalidPercentEncoding,
  /// Unexpected end of input.
  UnexpectedEof,
}

impl fmt::Display for UriError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | UriError::InvalidScheme => write!(f, "invalid scheme format"),
      | UriError::InvalidAuthority => write!(f, "invalid authority format"),
      | UriError::InvalidPath => write!(f, "invalid path format"),
      | UriError::InvalidQuery => write!(f, "invalid query format"),
      | UriError::InvalidFragment => write!(f, "invalid fragment format"),
      | UriError::InvalidPercentEncoding => write!(f, "invalid percent encoding"),
      | UriError::UnexpectedEof => write!(f, "unexpected end of input"),
    }
  }
}

#[cfg(test)]
impl std::error::Error for UriError {}

/// RFC2396-compliant URI parser.
pub struct UriParser;

impl UriParser {
  /// Decodes percent-encoded strings according to RFC2396.
  ///
  /// # Errors
  ///
  /// Returns `UriError::InvalidPercentEncoding` if the input contains invalid percent encoding.
  pub fn percent_decode(input: &str) -> Result<alloc::string::String, UriError> {
    let mut result = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
      if ch == '%' {
        // Need at least 2 more characters
        let hex1 = chars.next().ok_or(UriError::InvalidPercentEncoding)?;
        let hex2 = chars.next().ok_or(UriError::InvalidPercentEncoding)?;

        // Convert hex characters to byte
        let hex_digit = |c: char| -> Option<u8> {
          match c {
            | '0'..='9' => Some(c as u8 - b'0'),
            | 'a'..='f' => Some(c as u8 - b'a' + 10),
            | 'A'..='F' => Some(c as u8 - b'A' + 10),
            | _ => None,
          }
        };

        let byte1 = hex_digit(hex1).ok_or(UriError::InvalidPercentEncoding)?;
        let byte2 = hex_digit(hex2).ok_or(UriError::InvalidPercentEncoding)?;
        let byte = (byte1 << 4) | byte2;
        result.push(byte);
      } else {
        // Regular character - convert to bytes
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        result.extend_from_slice(encoded.as_bytes());
      }
    }

    // Convert bytes to string, validating UTF-8
    alloc::string::String::from_utf8(result).map_err(|_| UriError::InvalidPercentEncoding)
  }

  /// Validates a hostname according to RFC2396.
  ///
  /// Supports ASCII hostnames, IPv4 addresses, and IPv6 addresses (in brackets).
  ///
  /// # Errors
  ///
  /// Returns `UriError::InvalidAuthority` if the hostname is invalid.
  pub fn validate_hostname(hostname: &str) -> Result<(), UriError> {
    if hostname.is_empty() {
      return Err(UriError::InvalidAuthority);
    }

    // IPv6 address in brackets
    if hostname.starts_with('[') && hostname.ends_with(']') {
      let ipv6 = &hostname[1..hostname.len() - 1];
      // Basic IPv6 validation - check for valid hex digits, colons, and optional zone ID
      if ipv6.is_empty() {
        return Err(UriError::InvalidAuthority);
      }
      // Allow IPv6 format: hex digits, colons, and optional %zone
      let valid_ipv6 = ipv6.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '%' || c.is_ascii_alphanumeric());
      if !valid_ipv6 {
        return Err(UriError::InvalidAuthority);
      }
      return Ok(());
    }

    // IPv4 address validation (basic check)
    if hostname.split('.').count() == 4 {
      let is_ipv4 = hostname.split('.').all(|part| part.parse::<u8>().is_ok() && !part.is_empty());
      if is_ipv4 {
        return Ok(());
      }
    }

    // ASCII hostname validation
    // Hostnames can contain letters, digits, hyphens, and dots
    // Cannot start or end with hyphen or dot
    if hostname.starts_with('-') || hostname.ends_with('-') {
      return Err(UriError::InvalidAuthority);
    }
    if hostname.starts_with('.') || hostname.ends_with('.') {
      return Err(UriError::InvalidAuthority);
    }

    // Check for valid characters
    let valid = hostname.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_');
    if !valid {
      return Err(UriError::InvalidAuthority);
    }

    // Check length (RFC 1035: max 253 characters for domain name, but we allow up to 255)
    if hostname.len() > 255 {
      return Err(UriError::InvalidAuthority);
    }

    Ok(())
  }

  /// Parses a URI string into its components.
  ///
  /// # Errors
  ///
  /// Returns `UriError` if the input does not conform to RFC2396.
  pub fn parse(input: &str) -> Result<UriParts<'_>, UriError> {
    // Empty string is invalid
    if input.is_empty() {
      return Err(UriError::InvalidScheme);
    }

    let mut remaining = input;

    // Parse scheme (optional, but if present must be valid)
    let (scheme, after_scheme) = if remaining.starts_with("://") {
      // URI starting with "://" without scheme is invalid
      return Err(UriError::InvalidScheme);
    } else if let Some(colon_pos) = remaining.find(':') {
      let scheme_str = &remaining[..colon_pos];
      if scheme_str.is_empty() {
        return Err(UriError::InvalidScheme);
      }
      // Validate scheme: must start with letter and contain only letters, digits, +, -, .
      if !scheme_str.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        return Err(UriError::InvalidScheme);
      }
      if !scheme_str.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return Err(UriError::InvalidScheme);
      }
      remaining = &remaining[colon_pos + 1..];
      (Some(scheme_str), remaining)
    } else {
      (None, remaining)
    };

    // Parse authority and path
    let (authority, path_start) = if let Some(after_slashes) = after_scheme.strip_prefix("//") {
      // Find the end of authority (either /, ?, #, or end of string)
      let authority_end = after_slashes.find(['/', '?', '#']).unwrap_or(after_slashes.len());
      let authority_str = if authority_end > 0 { Some(&after_slashes[..authority_end]) } else { None };
      remaining = &after_slashes[authority_end..];
      (authority_str, remaining)
    } else {
      (None, after_scheme)
    };

    // Parse path (up to ? or #)
    let path_end = path_start.find(['?', '#']).unwrap_or(path_start.len());
    let path = if path_end > 0 { &path_start[..path_end] } else { "" };
    remaining = &path_start[path_end..];

    // Parse query (after ?)
    let query = if remaining.starts_with('?') {
      let query_start = &remaining[1..];
      let query_end = query_start.find('#').unwrap_or(query_start.len());
      remaining = &query_start[query_end..];
      if query_end > 0 { Some(&query_start[..query_end]) } else { None }
    } else {
      None
    };

    // Parse fragment (after #)
    let fragment = remaining.strip_prefix('#').filter(|&fragment_str| !fragment_str.is_empty());

    Ok(UriParts { scheme, authority, path, query, fragment })
  }
}

#[cfg(test)]
mod tests;
