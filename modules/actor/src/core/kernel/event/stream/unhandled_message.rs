//! Classic event-stream alias for unhandled messages.

use crate::core::kernel::event::stream::TypedUnhandledMessageEvent;

/// Classic-compatible alias for the unhandled-message event payload.
pub type UnhandledMessage = TypedUnhandledMessageEvent;
