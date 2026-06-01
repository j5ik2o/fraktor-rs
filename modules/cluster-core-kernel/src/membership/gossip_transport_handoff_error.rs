//! Errors reported while creating logical gossip transport handoff.

use alloc::boxed::Box;

use fraktor_remote_core_rs::address::UniqueAddress;

/// Logical transport handoff validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GossipTransportHandoffError {
  /// The target peer identity is not mapped to a transport endpoint.
  UnknownPeer {
    /// Unknown peer identity.
    peer: UniqueAddress,
  },
  /// The envelope deadline has already passed.
  DeadlineExpired {
    /// Deadline tick carried by the envelope.
    deadline_tick: u64,
    /// Current tick observed by the caller.
    now_tick:      u64,
  },
  /// The envelope identity does not match the expected local identity.
  InvalidIdentity {
    /// Expected local identity.
    expected: Box<UniqueAddress>,
    /// Actual identity carried by the handoff.
    actual:   Box<UniqueAddress>,
  },
  /// A transport payload kind tag is not recognized.
  UnknownPayloadKind {
    /// Unknown logical payload kind tag.
    tag: u8,
  },
}
