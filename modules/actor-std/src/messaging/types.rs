use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Owned message envelope specialised for `StdToolbox`.
pub type AnyMessage = fraktor_actor_core_rs::core::messaging::AnyMessageGeneric<StdToolbox>;
/// Borrowed message view specialised for `StdToolbox`.
pub type AnyMessageView<'a> = fraktor_actor_core_rs::core::messaging::AnyMessageViewGeneric<'a, StdToolbox>;
/// Ask-response handle specialised for `StdToolbox`.
pub type AskResponse = fraktor_actor_core_rs::core::messaging::AskResponseGeneric<StdToolbox>;
