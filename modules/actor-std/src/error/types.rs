use fraktor_utils_core_rs::std::runtime_toolbox::StdToolbox;

/// Send error specialised for `StdToolbox`.
pub type SendError = fraktor_actor_core_rs::core::error::SendError<StdToolbox>;
