//! Messaging package.
//!
//! This module contains message handling, type erasure, and Ask/Tell patterns.

mod actor_identity;
mod any_message;
mod any_message_view;
mod ask_error;
mod ask_response;
mod identify;
pub mod message_invoker;
mod status;
/// Internal system messages exchanged within the actor runtime.
pub mod system_message;

pub use actor_identity::ActorIdentity;
pub use any_message::AnyMessage;
pub use any_message_view::AnyMessageView;
pub use ask_error::AskError;
pub use ask_response::{AskResponse, AskResult};
pub use identify::Identify;
pub use status::Status;
