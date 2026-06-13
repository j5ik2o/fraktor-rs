//! Replica-count query protocol vocabulary.

/// Query requesting the current replica count including the local node.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GetReplicaCount;
