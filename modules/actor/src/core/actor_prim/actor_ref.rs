//! Actor reference handle module aggregation.

mod actor_ref_sender;
mod actor_ref_sender_shared;
mod ask_reply_sender;
mod base;
mod null_sender;
mod send_outcome;

pub use actor_ref_sender::ActorRefSender;
pub use actor_ref_sender_shared::{ActorRefSenderShared, ActorRefSenderSharedGeneric};
pub use ask_reply_sender::AskReplySender;
pub(crate) use ask_reply_sender::AskReplySenderGeneric;
pub use base::{ActorRef, ActorRefGeneric};
pub use null_sender::NullSender;
pub use send_outcome::SendOutcome;

#[cfg(test)]
mod tests;
