//! URI component structures.

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
