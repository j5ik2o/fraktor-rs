#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout, clippy::dbg_macro)]
#![deny(clippy::missing_errors_doc, clippy::missing_panics_doc)]

//! Standard library helpers for Cellactor runtime integrations.

pub use cellactor_actor_core_rs::{ActorSystemGeneric, Props};
pub use cellactor_utils_std_rs::{StdMutex, StdMutexFamily, StdToolbox};

/// 型エイリアス: std 環境向けツールボックスで動作する ActorSystem。
pub type StdActorSystem = ActorSystemGeneric<StdToolbox>;
