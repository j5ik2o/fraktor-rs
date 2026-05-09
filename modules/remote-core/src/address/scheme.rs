//! Actor path URI scheme for remote addresses.

/// Canonical scheme supported by the fraktor remoting layer.
///
/// Defined locally in `remote-core` so that the crate does not rely on a `pub use`
/// from `fraktor-actor-core-kernel-rs` (which would violate the workspace `no-parent-reexport`
/// lint that only permits re-exports from the direct parent module). The variants
/// intentionally mirror `fraktor_actor_core_kernel_rs::actor::actor_path::ActorPathScheme`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActorPathScheme {
  /// Local Fraktor transport.
  Fraktor,
  /// TCP transport compatible with Fraktor remoting.
  FraktorTcp,
}

impl ActorPathScheme {
  /// Returns the canonical scheme string.
  #[must_use]
  pub const fn as_str(&self) -> &'static str {
    match self {
      | ActorPathScheme::Fraktor => "fraktor",
      | ActorPathScheme::FraktorTcp => "fraktor.tcp",
    }
  }
}
