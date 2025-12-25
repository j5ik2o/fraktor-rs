//! Messaging package.
//!
//! This module contains message handling, type erasure, and Ask/Tell patterns.

mod any_message;
mod any_message_view;
mod ask_error;
mod ask_response;
pub mod message_invoker;
mod system_message;

pub use any_message::{AnyMessage, AnyMessageGeneric};
pub use any_message_view::{AnyMessageView, AnyMessageViewGeneric};
pub use ask_error::AskError;
pub use ask_response::{AskResponse, AskResponseGeneric, AskResult};
pub use system_message::{FailureClassification, FailureMessageSnapshot, FailurePayload, SystemMessage};
