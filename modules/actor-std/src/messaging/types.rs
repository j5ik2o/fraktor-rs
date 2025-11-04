use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Owned message envelope specialised for `StdToolbox`.
pub type AnyMessage = cellactor_actor_core_rs::messaging::AnyMessageGeneric<StdToolbox>;
/// Borrowed message view specialised for `StdToolbox`.
pub type AnyMessageView<'a> = cellactor_actor_core_rs::messaging::AnyMessageView<'a, StdToolbox>;
/// Ask-response handle specialised for `StdToolbox`.
pub type AskResponse = cellactor_actor_core_rs::messaging::AskResponseGeneric<StdToolbox>;
