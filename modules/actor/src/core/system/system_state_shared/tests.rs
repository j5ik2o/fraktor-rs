use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, sync_rwlock_like::SyncRwLockLike};

use super::SystemStateShared;
use crate::core::{
  actor::actor_ref::ActorRefGeneric,
  system::{GuardianKind, RegisterExtensionError, RegisterExtraTopLevelError, SystemState},
};

#[test]
fn register_extra_top_level_after_root_started_does_not_block_on_read_lock() {
  let shared = SystemStateShared::new(SystemState::new());
  shared.mark_root_started();

  let inner = shared.inner().clone();
  let shared_for_register = shared.clone();

  let (locked_tx, locked_rx) = std::sync::mpsc::channel::<()>();
  let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
  let (result_tx, result_rx) = std::sync::mpsc::channel();

  let reader = std::thread::spawn(move || {
    let _guard = inner.read();
    locked_tx.send(()).expect("locked");
    release_rx.recv().expect("release");
  });

  locked_rx.recv_timeout(Duration::from_secs(1)).expect("lock ready");

  let register = std::thread::spawn(move || {
    let result = shared_for_register.register_extra_top_level("metrics", ActorRefGeneric::null());
    result_tx.send(result).expect("result");
  });

  let early = result_rx.recv_timeout(Duration::from_millis(200)).ok();

  release_tx.send(()).expect("release send");
  reader.join().expect("reader join");
  register.join().expect("register join");

  assert!(early.is_some(), "root_started後のregisterはreadロック中でもブロックしないはず");
  assert!(matches!(early.unwrap(), Err(RegisterExtraTopLevelError::AlreadyStarted)));
}

#[test]
fn with_actor_path_registry_does_not_hold_outer_read_lock_while_running_callback() {
  let shared = SystemStateShared::new(SystemState::new());

  let shared_for_registry = shared.clone();
  let shared_for_write = shared.clone();

  let (callback_started_tx, callback_started_rx) = std::sync::mpsc::channel::<()>();
  let (callback_release_tx, callback_release_rx) = std::sync::mpsc::channel::<()>();
  let (write_result_tx, write_result_rx) = std::sync::mpsc::channel();

  let registry_thread = std::thread::spawn(move || {
    shared_for_registry.with_actor_path_registry(|_| {
      callback_started_tx.send(()).expect("started");
      callback_release_rx.recv().expect("release");
    });
  });

  callback_started_rx.recv_timeout(Duration::from_secs(1)).expect("callback should start quickly");

  let write_thread = std::thread::spawn(move || {
    let result = shared_for_write.register_extra_top_level("metrics", ActorRefGeneric::null());
    write_result_tx.send(result).expect("result");
  });

  let early = write_result_rx.recv_timeout(Duration::from_millis(200)).ok();

  callback_release_tx.send(()).expect("release send");
  registry_thread.join().expect("registry join");
  write_thread.join().expect("write join");

  assert!(early.is_some(), "with_actor_path_registry のコールバック中でも write がブロックしないはず");
  assert!(early.unwrap().is_ok());
}

#[test]
fn extension_or_insert_with_does_not_hold_outer_read_lock_while_running_factory() {
  use core::any::TypeId;

  struct TestExtension;

  let shared = SystemStateShared::new(SystemState::new());

  let shared_for_extension = shared.clone();
  let shared_for_write = shared.clone();

  let (factory_started_tx, factory_started_rx) = std::sync::mpsc::channel::<()>();
  let (factory_release_tx, factory_release_rx) = std::sync::mpsc::channel::<()>();
  let (extension_done_tx, extension_done_rx) = std::sync::mpsc::channel::<()>();
  let (write_result_tx, write_result_rx) = std::sync::mpsc::channel();

  let extension_thread = std::thread::spawn(move || {
    let _ext = shared_for_extension
      .extension_or_insert_with(TypeId::of::<TestExtension>(), || {
        factory_started_tx.send(()).expect("started");
        factory_release_rx.recv().expect("release");
        ArcShared::new(TestExtension)
      })
      .expect("extension");
    extension_done_tx.send(()).expect("done");
  });

  factory_started_rx.recv_timeout(Duration::from_secs(1)).expect("factory should start quickly");

  let write_thread = std::thread::spawn(move || {
    let result = shared_for_write.register_extra_top_level("metrics", ActorRefGeneric::null());
    write_result_tx.send(result).expect("result");
  });

  let early = write_result_rx.recv_timeout(Duration::from_millis(200)).ok();

  factory_release_tx.send(()).expect("release send");
  extension_done_rx.recv_timeout(Duration::from_secs(1)).expect("extension thread should finish");
  extension_thread.join().expect("extension join");
  write_thread.join().expect("write join");

  assert!(early.is_some(), "extension_or_insert_with の factory 実行中でも write がブロックしないはず");
  assert!(early.unwrap().is_ok());
}

#[test]
fn extension_or_insert_with_after_root_started_does_not_block_on_read_lock() {
  use core::any::TypeId;
  use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  };

  struct TestExtension;

  let shared = SystemStateShared::new(SystemState::new());

  let root_pid = shared.allocate_pid();
  shared.register_guardian_pid(GuardianKind::Root, root_pid);
  shared.mark_root_started();

  let inner = shared.inner().clone();
  let shared_for_extension = shared.clone();
  let factory_called = Arc::new(AtomicBool::new(false));
  let factory_called_for_thread = factory_called.clone();

  let (locked_tx, locked_rx) = std::sync::mpsc::channel::<()>();
  let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
  let (result_tx, result_rx) = std::sync::mpsc::channel();

  let reader = std::thread::spawn(move || {
    let _guard = inner.read();
    locked_tx.send(()).expect("locked");
    release_rx.recv().expect("release");
  });

  locked_rx.recv_timeout(Duration::from_secs(1)).expect("lock ready");

  let extension = std::thread::spawn(move || {
    let result = shared_for_extension
      .extension_or_insert_with(TypeId::of::<TestExtension>(), || {
        factory_called_for_thread.store(true, Ordering::SeqCst);
        ArcShared::new(TestExtension)
      })
      .map(|_| ());
    result_tx.send(result).expect("result");
  });

  let early = result_rx.recv_timeout(Duration::from_millis(200)).ok();

  release_tx.send(()).expect("release send");
  reader.join().expect("reader join");
  extension.join().expect("extension join");

  assert!(early.is_some(), "root_started後のextension登録はreadロック中でもブロックしないはず");
  assert!(matches!(early.unwrap(), Err(RegisterExtensionError::AlreadyStarted)));
  assert!(!factory_called.load(Ordering::SeqCst), "AlreadyStartedのときfactoryは呼ばれないはず");
}

#[test]
fn clear_guardian_does_not_block_on_read_lock() {
  let shared = SystemStateShared::new(SystemState::new());

  let root_pid = shared.allocate_pid();
  shared.register_guardian_pid(GuardianKind::Root, root_pid);

  let inner = shared.inner().clone();
  let shared_for_clear = shared.clone();

  let (locked_tx, locked_rx) = std::sync::mpsc::channel::<()>();
  let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
  let (result_tx, result_rx) = std::sync::mpsc::channel();

  let reader = std::thread::spawn(move || {
    let _guard = inner.read();
    locked_tx.send(()).expect("locked");
    release_rx.recv().expect("release");
  });

  locked_rx.recv_timeout(Duration::from_secs(1)).expect("lock ready");

  let clearer = std::thread::spawn(move || {
    let result = shared_for_clear.guardian_kind_by_pid(root_pid);
    if let Some(kind) = result {
      shared_for_clear.mark_guardian_stopped(kind);
    }
    result_tx.send(result).expect("result");
  });

  let early = result_rx.recv_timeout(Duration::from_millis(200)).ok();

  release_tx.send(()).expect("release send");
  reader.join().expect("reader join");
  clearer.join().expect("clearer join");

  assert!(early.is_some(), "clear_guardianはreadロック中でもブロックしないはず");
  assert!(matches!(early.unwrap(), Some(GuardianKind::Root)));
  assert!(!shared.guardian_alive(GuardianKind::Root));
}
