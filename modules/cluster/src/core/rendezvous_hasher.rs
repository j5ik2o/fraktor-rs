//! Rendezvous hashing for grain placement.

use alloc::string::String;

use crate::core::grain_key::GrainKey;

/// Selects an authority deterministically for a grain key.
pub struct RendezvousHasher;

impl RendezvousHasher {
  /// Chooses the authority with the highest hash score.
  pub fn select<'a>(authorities: &'a [String], key: &GrainKey) -> Option<&'a String> {
    authorities
      .iter()
      .max_by_key(|authority| Self::score(authority, key.value()))
  }

  fn score(authority: &str, key: &str) -> u64 {
    // Simple mixing inspired by FNV but tweaked for determinism.
    let mut hash = 0xcbf29ce484222325u64;
    for b in key.as_bytes().iter().chain(authority.as_bytes()) {
      hash ^= u64::from(*b);
      hash = hash.wrapping_mul(0x100000001b3);
      hash ^= hash >> 32;
    }
    hash
  }
}

#[cfg(test)]
mod tests;
