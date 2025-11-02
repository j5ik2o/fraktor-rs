#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout, clippy::dbg_macro)]
#![deny(clippy::missing_errors_doc, clippy::missing_panics_doc)]

//! Standard library helpers for Cellactor runtime integrations.

pub use cellactor_actor_core_rs::{
  ActorSystemGeneric, AskResponse as GenericAskResponse, ChildRef as GenericChildRef,
  DeadletterGeneric as GenericDeadletter, DispatcherConfig as GenericDispatcherConfig, Mailbox as GenericMailbox,
  MailboxConfig as GenericMailboxConfig, Props as GenericProps, SendError as GenericSendError,
  SupervisorOptions as GenericSupervisorOptions,
};
pub use cellactor_utils_std_rs::{StdMutex, StdMutexFamily, StdToolbox};

/// 型エイリアス: std 環境向けツールボックスで動作する ActorSystem。
pub type ActorSystem = ActorSystemGeneric<StdToolbox>;

/// 型エイリアス: std 向け Props。
pub type Props = cellactor_actor_core_rs::Props<StdToolbox>;

/// 型エイリアス: std 向け ActorContext。
pub type ActorContext<'a> = cellactor_actor_core_rs::ActorContext<'a, StdToolbox>;

/// 型エイリアス: std 向け ActorRef。
pub type ActorRef = cellactor_actor_core_rs::ActorRef<StdToolbox>;

/// 型エイリアス: std 向け ChildRef。
pub type ChildRef = cellactor_actor_core_rs::ChildRef<StdToolbox>;

/// 型エイリアス: std 向け AnyMessage。
pub type AnyMessage = cellactor_actor_core_rs::AnyMessage<StdToolbox>;

/// 型エイリアス: std 向け AnyMessageView。
pub type AnyMessageView<'a> = cellactor_actor_core_rs::AnyMessageView<'a, StdToolbox>;

/// 型エイリアス: std 向け ActorFuture。
pub type ActorFuture<T> = cellactor_actor_core_rs::ActorFuture<T, StdToolbox>;

/// 型エイリアス: std 向け ActorFutureListener。
pub type ActorFutureListener<'a, T> = cellactor_actor_core_rs::ActorFutureListener<'a, T, StdToolbox>;

/// 型エイリアス: std 向け AskResponse。
pub type AskResponse = cellactor_actor_core_rs::AskResponse<StdToolbox>;

/// 型エイリアス: std 向け SendError。
pub type SendError = cellactor_actor_core_rs::SendError<StdToolbox>;

/// 型エイリアス: std 向け Mailbox。
pub type Mailbox = cellactor_actor_core_rs::Mailbox<StdToolbox>;

/// 型エイリアス: std 向け Dispatcher。
pub type Dispatcher = cellactor_actor_core_rs::Dispatcher<StdToolbox>;

/// 型エイリアス: std 向け DispatcherConfig。
pub type DispatcherConfig = cellactor_actor_core_rs::DispatcherConfig<StdToolbox>;

/// 型エイリアス: std 向け EventStream。
pub type EventStream = cellactor_actor_core_rs::EventStreamGeneric<StdToolbox>;

/// 型エイリアス: std 向け EventStreamEvent。
pub type EventStreamEvent = cellactor_actor_core_rs::EventStreamEvent<StdToolbox>;

/// 型エイリアス: std 向け EventStreamSubscription。
pub type EventStreamSubscription = cellactor_actor_core_rs::EventStreamSubscriptionGeneric<StdToolbox>;

/// 型エイリアス: std 向け Deadletter。
pub type Deadletter = cellactor_actor_core_rs::DeadletterGeneric<StdToolbox>;

/// 型エイリアス: std 向け SystemState。
pub type SystemState = cellactor_actor_core_rs::SystemState<StdToolbox>;
