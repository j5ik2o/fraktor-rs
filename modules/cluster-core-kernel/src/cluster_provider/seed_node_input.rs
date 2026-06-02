//! Seed node input captured from provider lifecycle.

use alloc::{string::String, vec::Vec};

/// Seed authorities observed when a cluster provider starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedNodeInput {
  advertised_authority: String,
  seed_authorities:     Vec<String>,
}

impl SeedNodeInput {
  /// Creates seed node input for the advertised local authority.
  #[must_use]
  pub const fn new(advertised_authority: String, seed_authorities: Vec<String>) -> Self {
    Self { advertised_authority, seed_authorities }
  }

  /// Returns the advertised local authority.
  #[must_use]
  pub const fn advertised_authority(&self) -> &str {
    self.advertised_authority.as_str()
  }

  /// Returns candidate seed authorities.
  #[must_use]
  pub const fn seed_authorities(&self) -> &[String] {
    self.seed_authorities.as_slice()
  }
}
