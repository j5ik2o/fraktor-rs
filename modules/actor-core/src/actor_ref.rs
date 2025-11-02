//! Actor reference handle module aggregation.

mod actor_ref_impl;
mod actor_ref_sender;
mod ask_reply_sender;
mod null_sender;

pub use actor_ref_impl::ActorRef;
#[allow(unused_imports)]
pub use actor_ref_sender::ActorRefSender;
#[allow(unused_imports)]
pub use ask_reply_sender::AskReplySender;
#[allow(unused_imports)]
pub use null_sender::NullSender;

#[cfg(test)]
mod tests;
