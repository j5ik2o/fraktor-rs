#[cfg(test)]
mod tests;

/// Deployment scope for classic actor deployment descriptors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
  /// Deploy the actor locally.
  #[default]
  Local,
  /// Deploy the actor remotely.
  Remote,
}
