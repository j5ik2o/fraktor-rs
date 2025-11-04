use cellactor_utils_std_rs::StdToolbox;

/// Owned message envelope specialised for `StdToolbox`.
pub type AnyMessage = cellactor_actor_core_rs::messaging::AnyMessage<StdToolbox>;
/// Borrowed message view specialised for `StdToolbox`.
pub type AnyMessageView<'a> = cellactor_actor_core_rs::messaging::AnyMessageView<'a, StdToolbox>;
/// Ask-response handle specialised for `StdToolbox`.
pub type AskResponse = cellactor_actor_core_rs::messaging::AskResponse<StdToolbox>;
