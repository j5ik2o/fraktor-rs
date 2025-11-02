//! Actor reference handle.

mod actor_ref_impl;
mod actor_ref_sender;
mod ask_reply_sender;
mod null_sender;

pub use actor_ref_impl::ActorRef;
pub use actor_ref_sender::ActorRefSender;

#[cfg(test)]
mod tests;
