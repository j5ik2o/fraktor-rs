//! Errors raised while dispatching through the sharding router.

use super::GrainCallError;
use crate::activation::ClusterIdentityError;

/// Errors raised while resolving or sending through the sharding router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardingDispatchError {
  /// Entity id could not be derived from the message.
  EntityIdUnderivable,
  /// Derived identity was rejected by the kernel validation rules.
  InvalidIdentity(ClusterIdentityError),
  /// Underlying grain call failed.
  Call(GrainCallError),
}
