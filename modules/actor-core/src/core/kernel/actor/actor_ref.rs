//! Actor reference handle module aggregation.

mod actor_ref_sender;
mod actor_ref_sender_shared;
mod actor_ref_sender_shared_factory;
mod ask_reply_sender;
mod base;
pub mod dead_letter;
mod null_sender;
mod send_outcome;

pub use actor_ref_sender::ActorRefSender;
pub use actor_ref_sender_shared::ActorRefSenderShared;
pub use actor_ref_sender_shared_factory::ActorRefSenderSharedFactory;
pub use ask_reply_sender::AskReplySender;
pub use base::ActorRef;
pub use null_sender::NullSender;
pub use send_outcome::SendOutcome;

#[cfg(test)]
mod tests;
