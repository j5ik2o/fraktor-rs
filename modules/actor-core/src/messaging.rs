//! Messaging package.
//!
//! This module contains message handling, type erasure, and Ask/Tell patterns.

mod any_message;
mod any_message_view;
mod ask_response;
pub mod message_invoker;
mod system_message;

pub use any_message::AnyMessage;
pub use any_message_view::AnyMessageView;
pub use ask_response::AskResponse;
pub use system_message::SystemMessage;
