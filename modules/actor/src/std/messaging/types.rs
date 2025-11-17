use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Owned message envelope specialised for `StdToolbox`.
pub type AnyMessage = crate::core::messaging::AnyMessageGeneric<StdToolbox>;
/// Borrowed message view specialised for `StdToolbox`.
pub type AnyMessageView<'a> = crate::core::messaging::AnyMessageViewGeneric<'a, StdToolbox>;
/// Ask-response handle specialised for `StdToolbox`.
pub type AskResponse = crate::core::messaging::AskResponseGeneric<StdToolbox>;
