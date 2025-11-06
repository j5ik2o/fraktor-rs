//! Actor reference handle module aggregation.

mod base;
mod actor_ref_sender;
mod ask_reply_sender;
mod null_sender;

pub use base::{ActorRef, ActorRefGeneric};
pub use actor_ref_sender::ActorRefSender;
pub use ask_reply_sender::AskReplySender;
pub use null_sender::NullSender;

#[cfg(test)]
mod tests;
