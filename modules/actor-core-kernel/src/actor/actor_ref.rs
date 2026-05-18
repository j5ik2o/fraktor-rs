//! Actor reference handle module aggregation.

mod actor_ref_sender;
mod actor_ref_sender_shared;
mod ask_reply_sender;
mod base;
pub mod dead_letter;
mod null_sender;
mod send_outcome;

pub use actor_ref_sender::ActorRefSender;
pub use actor_ref_sender_shared::ActorRefSenderShared;
pub use ask_reply_sender::AskReplySender;
pub use base::ActorRef;
pub use null_sender::NullSender;
pub use send_outcome::SendOutcome;

#[cfg(test)]
#[path = "actor_ref_test.rs"]
mod tests;
