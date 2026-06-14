//! Typed cluster management commands.

use alloc::{string::String, vec::Vec};

use fraktor_cluster_core_kernel_rs::extension::ClusterError;

use crate::Cluster;

/// Commands that execute membership operations through [`Cluster`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusterCommand {
  /// Join a single member authority.
  Join {
    /// Member authority.
    address: String,
  },
  /// Join multiple seed-node authorities in order.
  JoinSeedNodes {
    /// Seed-node authorities.
    addresses: Vec<String>,
  },
  /// Request graceful leave for a member authority.
  Leave {
    /// Member authority.
    address: String,
  },
  /// Mark a member authority as down.
  Down {
    /// Member authority.
    address: String,
  },
  /// Initiate full-cluster shutdown preparation.
  PrepareForFullClusterShutdown,
}

impl ClusterCommand {
  /// Applies this command to the supplied typed cluster facade.
  ///
  /// # Errors
  ///
  /// Returns the first kernel cluster error encountered while executing the command.
  pub fn apply_to(&self, cluster: &Cluster) -> Result<(), ClusterError> {
    match self {
      | Self::Join { address } => cluster.join(address),
      | Self::JoinSeedNodes { addresses } => {
        for address in addresses {
          cluster.join(address)?;
        }
        Ok(())
      },
      | Self::Leave { address } => cluster.leave(address),
      | Self::Down { address } => cluster.down(address),
      | Self::PrepareForFullClusterShutdown => cluster.prepare_for_full_cluster_shutdown(),
    }
  }
}
