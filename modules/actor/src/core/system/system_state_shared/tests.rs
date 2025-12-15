use core::time::Duration;

use fraktor_utils_rs::core::sync::sync_rwlock_like::SyncRwLockLike;

use super::SystemStateShared;
use crate::core::{
  actor_prim::actor_ref::ActorRefGeneric,
  system::{RegisterExtraTopLevelError, SystemState},
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
