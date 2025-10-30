use core::task::Waker;

use cellactor_utils_core_rs::sync::ArcShared;

use super::{
  dispatch_executor::DispatchExecutor, dispatch_handle::DispatchHandle, dispatcher_core::DispatcherCore,
  dispatcher_state::DispatcherState, inline_executor::InlineExecutor, schedule_waker::ScheduleWaker,
};
use crate::{any_message::AnyOwnedMessage, mailbox::Mailbox, send_error::SendError, system_message::SystemMessage};

/// メールボックス処理を管理するディスパッチャ。
pub struct Dispatcher {
  core: ArcShared<DispatcherCore>,
}

impl Dispatcher {
  /// メールボックスと実行戦略から新しいディスパッチャを生成する。
  #[must_use]
  pub fn new(mailbox: ArcShared<Mailbox>, executor: ArcShared<dyn DispatchExecutor>) -> Self {
    let throughput = mailbox.throughput_limit();
    let core = ArcShared::new(DispatcherCore::new(mailbox, executor, throughput));
    Self::from_core(core)
  }

  /// インライン実行戦略を用いたディスパッチャを作成する。
  #[must_use]
  pub fn with_inline_executor(mailbox: ArcShared<Mailbox>) -> Self {
    Self::new(mailbox, ArcShared::new(InlineExecutor::new()))
  }

  /// インボーカーを登録する。
  pub fn register_invoker(&self, invoker: ArcShared<dyn crate::message_invoker::MessageInvoker>) {
    self.core.register_invoker(invoker);
  }

  /// ユーザーメッセージをキューに追加する。
  pub fn enqueue_user(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    DispatcherCore::enqueue_user(&self.core, message)
  }

  /// システムメッセージをキューに追加する。
  pub fn enqueue_system(&self, message: SystemMessage) -> Result<(), SendError> {
    DispatcherCore::enqueue_system(&self.core, message)
  }

  /// スケジューラに実行を要求する。
  pub fn schedule(&self) {
    let should_run = {
      let core_ref = &*self.core;
      DispatcherState::compare_exchange(DispatcherState::Idle, DispatcherState::Running, core_ref.state()).is_ok()
    };

    if should_run {
      let executor = self.core.executor().clone();
      executor.execute(DispatchHandle::new(self.core.clone()));
    }
  }

  /// メールボックス参照を取得する。
  #[must_use]
  pub fn mailbox(&self) -> ArcShared<Mailbox> {
    self.core.mailbox().clone()
  }

  /// メールボックス待機用ワーカを生成する。
  #[must_use]
  pub fn create_waker(&self) -> Waker {
    ScheduleWaker::into_waker(self.core.clone())
  }

  pub(super) fn from_core(core: ArcShared<DispatcherCore>) -> Self {
    Self { core }
  }
}

impl Clone for Dispatcher {
  fn clone(&self) -> Self {
    Self { core: self.core.clone() }
  }
}
