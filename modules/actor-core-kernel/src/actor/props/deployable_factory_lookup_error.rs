use alloc::string::String;

use crate::actor::props::DeployableFactoryError;

/// Failure raised while resolving deployable actor props on the target node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeployableFactoryLookupError {
  /// No factory is registered for the requested stable id.
  UnknownFactoryId(String),
  /// The registered factory rejected the deserialized payload.
  FactoryRejected(DeployableFactoryError),
}
