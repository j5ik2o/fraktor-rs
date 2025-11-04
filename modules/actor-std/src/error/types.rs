use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Send error specialised for `StdToolbox`.
pub type SendError = cellactor_actor_core_rs::error::SendError<StdToolbox>;
