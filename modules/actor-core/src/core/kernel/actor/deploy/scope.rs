#[cfg(test)]
mod tests;

use super::RemoteScope;

/// Deployment scope for classic actor deployment descriptors.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Scope {
  /// Deploy the actor locally.
  #[default]
  Local,
  /// Deploy the actor on a specific remote node.
  Remote(RemoteScope),
}
