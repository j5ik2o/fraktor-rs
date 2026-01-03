//! Endpoint association FSM and coordinator for managing remote connections.
//!
//! This module tracks the association state of remote endpoints through a
//! command-driven finite state machine, managing handshake transitions,
//! quarantine states, and deferred message queues.

mod command;
mod coordinator;
mod coordinator_shared;
mod effect;
mod quarantine_reason;
mod result;
mod state;

pub use command::EndpointAssociationCommand;
pub use coordinator::EndpointAssociationCoordinator;
pub use coordinator_shared::{EndpointAssociationCoordinatorShared, EndpointAssociationCoordinatorSharedGeneric};
pub use effect::EndpointAssociationEffect;
pub use quarantine_reason::QuarantineReason;
pub use result::EndpointAssociationResult;
pub use state::AssociationState;
