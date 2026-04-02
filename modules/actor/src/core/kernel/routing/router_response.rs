//! Responses from a router actor.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use super::routee::Routee;

/// Responses to router management commands.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.Routees`.
#[derive(Clone, Debug)]
pub enum RouterResponse {
  /// The current list of routees in the router.
  Routees(Vec<Routee>),
}
