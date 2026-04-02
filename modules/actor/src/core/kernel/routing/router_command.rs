//! Management commands for a router actor.

#[cfg(test)]
mod tests;

use super::routee::Routee;

/// Commands for dynamically modifying a router's routee set.
///
/// Corresponds to Pekko's router management messages:
/// `GetRoutees`, `AddRoutee`, `RemoveRoutee`, `AdjustPoolSize`.
#[derive(Clone, Debug)]
pub enum RouterCommand {
  /// Requests the current list of routees.
  GetRoutees,
  /// Adds a routee to the router.
  AddRoutee(Routee),
  /// Removes a routee from the router.
  RemoveRoutee(Routee),
  /// Adjusts the pool size by the given delta (may be negative).
  AdjustPoolSize(i32),
}
