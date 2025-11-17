//! Actor reference handle module aggregation.

mod actor_ref_sender;
mod ask_reply_sender;
mod base;
mod null_sender;

pub use actor_ref_sender::ActorRefSender;
pub use ask_reply_sender::AskReplySender;
pub use base::{ActorRef, ActorRefGeneric};
pub use null_sender::NullSender;

#[cfg(test)]
mod tests;
