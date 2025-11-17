use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Send error specialised for `StdToolbox`.
pub type SendError = crate::core::error::SendError<StdToolbox>;
