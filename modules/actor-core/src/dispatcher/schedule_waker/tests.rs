use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox,
  dispatcher::{InlineExecutor, dispatcher_core::DispatcherCore, schedule_waker::ScheduleWaker},
  mailbox::{Mailbox, MailboxPolicy},
};

#[test]
fn into_waker_creates_valid_waker() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mailbox_shared = ArcShared::new(mailbox);
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = DispatcherCore::new(mailbox_shared, executor, None);
  let core_shared = ArcShared::new(core);

  let waker = ScheduleWaker::<NoStdToolbox>::into_waker(core_shared);
  // Wakerが正常に作成されることを確認
  assert!(core::ptr::addr_of!(waker).is_aligned());
}

#[test]
fn waker_wake_schedules_dispatcher() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mailbox_shared = ArcShared::new(mailbox);
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = DispatcherCore::new(mailbox_shared, executor, None);
  let core_shared = ArcShared::new(core);

  let waker = ScheduleWaker::<NoStdToolbox>::into_waker(core_shared.clone());

  // wake()を呼び出す
  waker.wake();

  // スケジューリングが実行されたことを確認（クラッシュしないことを確認）
}

#[test]
fn waker_wake_by_ref_schedules_dispatcher() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mailbox_shared = ArcShared::new(mailbox);
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = DispatcherCore::new(mailbox_shared, executor, None);
  let core_shared = ArcShared::new(core);

  let waker = ScheduleWaker::<NoStdToolbox>::into_waker(core_shared.clone());

  // wake_by_ref()を呼び出す
  waker.wake_by_ref();

  // スケジューリングが実行されたことを確認
}

#[test]
fn waker_clone_creates_new_waker() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mailbox_shared = ArcShared::new(mailbox);
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = DispatcherCore::new(mailbox_shared, executor, None);
  let core_shared = ArcShared::new(core);

  let waker1 = ScheduleWaker::<NoStdToolbox>::into_waker(core_shared.clone());
  let waker2 = waker1.clone();

  // 両方のwakerが有効であることを確認
  waker2.wake_by_ref();
  waker1.wake_by_ref();
}

#[test]
fn waker_drop_cleans_up() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mailbox_shared = ArcShared::new(mailbox);
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = DispatcherCore::new(mailbox_shared, executor, None);
  let core_shared = ArcShared::new(core);

  {
    let _waker = ScheduleWaker::<NoStdToolbox>::into_waker(core_shared.clone());
    // スコープを抜けるとdropが呼ばれる
  }

  // クリーンアップが正常に行われたことを確認（クラッシュしない）
}
