//! Describes the local authority a transport listener should bind to.

use alloc::{format, string::String};

/// Binding information for a transport listener.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportBind {
  authority: String,
}

impl TransportBind {
  /// Creates a new bind definition using `host[:port]` semantics.
  #[must_use]
  pub fn new(host: impl Into<String>, port: Option<u16>) -> Self {
    let host = host.into();
    let authority = match port {
      | Some(port) => format!("{host}:{port}"),
      | None => host,
    };
    Self { authority }
  }

  /// Returns the canonical authority string.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }
}
