/// Owned message envelope for the standard runtime.
pub type AnyMessage = crate::core::messaging::AnyMessage;
/// Identity reply for classic actor discovery.
pub type ActorIdentity = crate::core::messaging::ActorIdentity;
/// Borrowed message view for the standard runtime.
pub type AnyMessageView<'a> = crate::core::messaging::AnyMessageView<'a>;
/// Ask-response handle for the standard runtime.
pub type AskResponse = crate::core::messaging::AskResponse;
/// Result type for ask operations, for the standard runtime.
pub type AskResult = crate::core::messaging::AskResult;
/// Identify request for classic actor discovery.
pub type Identify = crate::core::messaging::Identify;
/// Classic status reply envelope.
pub type Status = crate::core::messaging::Status;
