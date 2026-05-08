use alloc::{string::ToString, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::SystemStateShared;
use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::ActorRef,
    error::ActorError,
    messaging::{AnyMessageView, system_message::FailurePayload},
    props::Props,
  },
  event::{
    logging::{DefaultLoggingFilter, LogLevel},
    stream::{EventStreamEvent, EventStreamSubscriber, tests::subscriber_handle},
  },
  system::{
    RegisterExtraTopLevelError,
    guardian::GuardianKind,
    state::system_state::{FailureOutcome, SystemState},
  },
};

struct LogRecorder {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl LogRecorder {
  fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for LogRecorder {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

fn assert_operation_does_not_block_on_read_lock(
  shared: SystemStateShared,
  operation_name: &'static str,
  operation: impl FnOnce(SystemStateShared) + Send + 'static,
) {
  let inner = shared.inner.clone();
  let shared_for_operation = shared.clone();

  let (locked_tx, locked_rx) = std::sync::mpsc::channel::<()>();
  let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
  let (result_tx, result_rx) = std::sync::mpsc::channel::<()>();

  let reader = std::thread::spawn(move || {
    inner.with_read(|_| {
      locked_tx.send(()).expect("locked");
      release_rx.recv().expect("release");
    });
  });

  locked_rx.recv_timeout(Duration::from_secs(1)).expect("lock ready");

  let worker = std::thread::spawn(move || {
    operation(shared_for_operation);
    result_tx.send(()).expect("result");
  });

  let early = result_rx.recv_timeout(Duration::from_millis(200)).ok();

  release_tx.send(()).expect("release send");
  reader.join().expect("reader join");
  worker.join().expect("worker join");

  assert!(early.is_some(), "{operation_name} は outer read lock 中でもブロックしないはず");
}

fn root_guardian_cell(shared: SystemStateShared, pid: Pid) -> ArcShared<ActorCell> {
  let props = Props::from_fn(|| GuardianProbeActor);
  ActorCell::create(shared, pid, None, "root".to_string(), &props).expect("root guardian cell")
}

#[test]
fn register_extra_top_level_after_root_started_does_not_block_on_read_lock() {
  let shared = SystemStateShared::new(SystemState::new());
  shared.mark_root_started();

  let inner = shared.inner.clone();
  let shared_for_register = shared.clone();

  let (locked_tx, locked_rx) = std::sync::mpsc::channel::<()>();
  let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
  let (result_tx, result_rx) = std::sync::mpsc::channel();

  let reader = std::thread::spawn(move || {
    inner.with_read(|_| {
      locked_tx.send(()).expect("locked");
      release_rx.recv().expect("release");
    });
  });

  locked_rx.recv_timeout(Duration::from_secs(1)).expect("lock ready");

  let register = std::thread::spawn(move || {
    let result = shared_for_register.register_extra_top_level("metrics", ActorRef::null());
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
    let result = shared_for_write.register_extra_top_level("metrics", ActorRef::null());
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
    let _ext = shared_for_extension.extension_or_insert_with(TypeId::of::<TestExtension>(), || {
      factory_started_tx.send(()).expect("started");
      factory_release_rx.recv().expect("release");
      ArcShared::new(TestExtension)
    });
    extension_done_tx.send(()).expect("done");
  });

  factory_started_rx.recv_timeout(Duration::from_secs(1)).expect("factory should start quickly");

  let write_thread = std::thread::spawn(move || {
    let result = shared_for_write.register_extra_top_level("metrics", ActorRef::null());
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
fn extension_or_insert_with_after_root_started_succeeds() {
  use core::any::TypeId;

  struct TestExtension;

  let shared = SystemStateShared::new(SystemState::new());

  shared.mark_root_started();

  // Pekko compatibility: extensions can be registered at any time (putIfAbsent semantics).
  let _result = shared.extension_or_insert_with(TypeId::of::<TestExtension>(), || ArcShared::new(TestExtension));
}

#[test]
fn extension_or_insert_with_returns_registered_instance_when_concurrent_factory_loses_race() {
  use core::any::TypeId;

  struct TestExtension;

  let shared = SystemStateShared::new(SystemState::new());

  let shared_for_first = shared.clone();
  let shared_for_second = shared.clone();

  let (first_factory_started_tx, first_factory_started_rx) = std::sync::mpsc::channel::<()>();
  let (release_first_factory_tx, release_first_factory_rx) = std::sync::mpsc::channel::<()>();
  let (second_factory_started_tx, second_factory_started_rx) = std::sync::mpsc::channel::<()>();
  let (first_result_tx, first_result_rx) = std::sync::mpsc::channel();
  let (second_result_tx, second_result_rx) = std::sync::mpsc::channel();

  let first_thread = std::thread::spawn(move || {
    let extension = shared_for_first.extension_or_insert_with(TypeId::of::<TestExtension>(), || {
      first_factory_started_tx.send(()).expect("first factory started");
      release_first_factory_rx.recv().expect("release first factory");
      ArcShared::new(TestExtension)
    });
    first_result_tx.send(extension).expect("first result");
  });

  first_factory_started_rx.recv_timeout(Duration::from_secs(1)).expect("first factory should start quickly");

  let second_thread = std::thread::spawn(move || {
    let extension = shared_for_second.extension_or_insert_with(TypeId::of::<TestExtension>(), || {
      second_factory_started_tx.send(()).expect("second factory started");
      ArcShared::new(TestExtension)
    });
    second_result_tx.send(extension).expect("second result");
  });

  second_factory_started_rx.recv_timeout(Duration::from_secs(1)).expect("second factory should start quickly");

  let second = second_result_rx.recv_timeout(Duration::from_secs(1)).expect("second result should arrive");

  release_first_factory_tx.send(()).expect("release first factory");

  let first = first_result_rx.recv_timeout(Duration::from_secs(1)).expect("first result should arrive");

  first_thread.join().expect("first join");
  second_thread.join().expect("second join");

  assert!(ArcShared::ptr_eq(&first, &second), "競合した extension 登録は同じ共有インスタンスを返すべき");

  let registered =
    shared.extension::<TestExtension>(TypeId::of::<TestExtension>()).expect("registered extension should exist");

  assert!(ArcShared::ptr_eq(&registered, &first), "返却値は登録済みインスタンスと一致するべき");
}

#[test]
fn clear_guardian_does_not_block_on_read_lock() {
  let shared = SystemStateShared::new(SystemState::new());

  let root_pid = shared.allocate_pid();
  let root_cell = root_guardian_cell(shared.clone(), root_pid);
  shared.set_root_guardian(&root_cell);

  let inner = shared.inner.clone();
  let shared_for_clear = shared.clone();

  let (locked_tx, locked_rx) = std::sync::mpsc::channel::<()>();
  let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
  let (result_tx, result_rx) = std::sync::mpsc::channel();

  let reader = std::thread::spawn(move || {
    inner.with_read(|_| {
      locked_tx.send(()).expect("locked");
      release_rx.recv().expect("release");
    });
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

struct GuardianProbeActor;

impl Actor for GuardianProbeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn atomic_state_transitions_do_not_block_on_read_lock() {
  assert_operation_does_not_block_on_read_lock(
    SystemStateShared::new(SystemState::new()),
    "mark_root_started",
    |shared| shared.mark_root_started(),
  );

  assert_operation_does_not_block_on_read_lock(
    SystemStateShared::new(SystemState::new()),
    "begin_termination",
    |shared| {
      assert!(shared.begin_termination());
    },
  );

  assert_operation_does_not_block_on_read_lock(
    SystemStateShared::new(SystemState::new()),
    "mark_terminated",
    |shared| shared.mark_terminated(),
  );
}

#[test]
fn failure_accounting_does_not_block_on_read_lock() {
  let shared = SystemStateShared::new(SystemState::new());
  let child = shared.allocate_pid();
  assert_operation_does_not_block_on_read_lock(shared, "record_failure_outcome", move |shared| {
    let payload = FailurePayload::from_error(child, &ActorError::recoverable("boom"), None, Duration::ZERO);
    shared.record_failure_outcome(child, FailureOutcome::Stop, &payload);
  });

  let shared = SystemStateShared::new(SystemState::new());
  let child = shared.allocate_pid();
  assert_operation_does_not_block_on_read_lock(shared, "report_failure", move |shared| {
    let payload = FailurePayload::from_error(child, &ActorError::recoverable("boom"), None, Duration::ZERO);
    shared.report_failure(payload);
  });
}

#[test]
fn emit_log_respects_default_logging_filter_threshold() {
  let shared = SystemStateShared::new(SystemState::new());
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(LogRecorder::new(events.clone()));
  let _subscription = shared.event_stream().subscribe(&subscriber);
  let pid = shared.allocate_pid();

  shared.set_logging_filter(DefaultLoggingFilter::new(LogLevel::Error));

  shared.emit_log(LogLevel::Warn, "filtered warn".into(), Some(pid), None);
  shared.emit_log(LogLevel::Error, "accepted error".into(), Some(pid), None);

  let events = events.lock().clone();
  assert!(!events.iter().any(|event| {
    matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Warn && log.message() == "filtered warn")
  }));
  assert!(events.iter().any(|event| {
    matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Error && log.message() == "accepted error")
  }));
}
