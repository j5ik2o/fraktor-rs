#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout, clippy::dbg_macro)]
#![deny(clippy::missing_errors_doc, clippy::missing_panics_doc)]

//! Standard library helpers for Cellactor runtime integrations.

pub use cellactor_actor_core_rs::*;

mod dispatcher_config_ext;
mod props_ext;
mod runtime_mutex;
mod tokio_dispatch_executor;

pub use dispatcher_config_ext::TokioDispatcherConfigExt;
pub use props_ext::TokioPropsExt;
pub use runtime_mutex::ActorRuntimeMutex;
pub use tokio_dispatch_executor::TokioDispatchExecutor;
