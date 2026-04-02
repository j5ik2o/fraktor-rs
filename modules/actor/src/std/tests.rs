use alloc::vec::Vec;
use std::path::{Path, PathBuf};

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::kernel::{
  actor::ActorContext,
  event::{
    logging::LogLevel,
    stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  },
  system::ActorSystem,
};

const REMOVED_STD_ALIAS_FILES: &[&str] = &[
  "src/std/dead_letter.rs",
  "src/std/error.rs",
  "src/std/futures.rs",
  "src/std/messaging.rs",
  "src/std/actor.rs",
  "src/std/dispatch/mailbox.rs",
  "src/std/dispatch/dispatcher/types.rs",
  "src/std/event/stream/types.rs",
  "src/std/props.rs",
  "src/std/typed/actor.rs",
  "src/std/typed/behavior.rs",
  "src/std/typed/spawn_protocol.rs",
  "src/std/typed/stash_buffer.rs",
  "src/std/typed/typed_ask_future.rs",
  "src/std/typed/typed_ask_response.rs",
];

const REMOVED_UNWIRED_STD_IO_PATHS: &[&str] = &[
  "src/std/io/connection_closed/tests.rs",
  "src/std/io/dns_command/tests.rs",
  "src/std/io/dns_event/tests.rs",
  "src/std/io/dns_ext/tests.rs",
  "src/std/io/tcp_command/tests.rs",
  "src/std/io/tcp_event/tests.rs",
  "src/std/io/tcp_ext/tests.rs",
  "src/std/io/tcp_socket_option/tests.rs",
  "src/std/io/udp_command/tests.rs",
  "src/std/io/udp_event/tests.rs",
  "src/std/io/udp_ext/tests.rs",
  "src/std/io/udp_socket_option/tests.rs",
];

const REQUIRED_ACTOR_EXAMPLE_FILES: &[&str] =
  &["examples/typed_event_stream.rs", "examples/classic_timers.rs", "examples/classic_logging.rs"];

const REQUIRED_ACTOR_EXAMPLE_NAMES: &[&str] = &["typed_event_stream", "classic_timers", "classic_logging"];

struct NoopSubscriber;

impl crate::std::event::stream::EventStreamSubscriber for NoopSubscriber {
  fn on_event(&mut self, _event: &EventStreamEvent) {}
}

struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

#[test]
fn removed_std_alias_files_stay_deleted() {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

  for relative_path in REMOVED_STD_ALIAS_FILES {
    let path = manifest_dir.join(relative_path);
    assert!(!path.exists(), "削除済み alias ファイルが復活しています: {}", display_relative_path(manifest_dir, &path));
  }
}

#[test]
fn unfinished_std_io_tree_stays_deleted_until_module_is_wired() {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

  for relative_path in REMOVED_UNWIRED_STD_IO_PATHS {
    let path = manifest_dir.join(relative_path);
    assert!(!path.exists(), "未配線の std/io ツリーが復活しています: {}", display_relative_path(manifest_dir, &path));
  }
}

#[test]
fn std_public_modules_expose_only_live_entry_points() {
  let _behaviors = core::marker::PhantomData::<crate::std::typed::Behaviors>;
  let _log_options = core::marker::PhantomData::<crate::core::typed::LogOptions>;
  let _actor_log_marker = core::marker::PhantomData::<crate::std::event::logging::ActorLogMarker>;
  let _actor_logging = core::marker::PhantomData::<crate::std::event::logging::ActorLogging>;
  let _diagnostic_actor_logging = core::marker::PhantomData::<crate::std::event::logging::DiagnosticActorLogging>;
  let _logging_adapter = core::marker::PhantomData::<crate::std::event::logging::LoggingAdapter>;
  let _logging_receive = core::marker::PhantomData::<crate::std::event::logging::LoggingReceive>;
  let _tracing_subscriber = core::marker::PhantomData::<crate::std::event::logging::TracingLoggerSubscriber>;
  let _shared = core::marker::PhantomData::<crate::std::event::stream::EventStreamSubscriberShared>;

  let _subscriber = crate::std::event::stream::subscriber_handle(NoopSubscriber);
}

#[test]
fn std_logging_module_exposes_classic_logging_family() {
  let _actor_log_marker = core::marker::PhantomData::<crate::std::event::logging::ActorLogMarker>;
  let _actor_logging = core::marker::PhantomData::<crate::std::event::logging::ActorLogging>;
  let _diagnostic_actor_logging = core::marker::PhantomData::<crate::std::event::logging::DiagnosticActorLogging>;
  let _logging_adapter = core::marker::PhantomData::<crate::std::event::logging::LoggingAdapter>;
  let _logging_receive = core::marker::PhantomData::<crate::std::event::logging::LoggingReceive>;
  let _tracing_subscriber = core::marker::PhantomData::<crate::std::event::logging::TracingLoggerSubscriber>;
}

#[test]
fn classic_actor_context_log_emits_context_bound_log_event() {
  // Given: event stream を購読した classic actor context がある
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);
  let mut context = ActorContext::new(&system, pid);
  context.set_logger_name("classic.actor.test");

  // When: classic actor context から直接 log() を呼ぶ
  context.log(LogLevel::Info, "classic context message");

  // Then: actor context の pid/logger_name を持つ LogEvent が publish される
  let events = events.lock().clone();
  assert!(events.iter().any(|event| {
    matches!(
      event,
      EventStreamEvent::Log(log)
        if log.level() == LogLevel::Info
          && log.message() == "classic context message"
          && log.origin() == Some(pid)
          && log.logger_name() == Some("classic.actor.test")
    )
  }));
}

#[test]
fn actor_examples_cover_phase2_and_classic_logging_surfaces() {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let cargo_toml = std::fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("Cargo.toml should be readable");

  for relative_path in REQUIRED_ACTOR_EXAMPLE_FILES {
    let path = manifest_dir.join(relative_path);
    assert!(path.exists(), "必須 example が不足しています: {}", display_relative_path(manifest_dir, &path));
  }

  for example_name in REQUIRED_ACTOR_EXAMPLE_NAMES {
    assert!(
      cargo_toml.contains(&format!("name = \"{example_name}\"")),
      "Cargo.toml に example 定義がありません: {example_name}"
    );
  }
}

fn display_relative_path(base: &Path, path: &Path) -> String {
  path.strip_prefix(base).map(PathBuf::from).unwrap_or_else(|_| path.to_path_buf()).display().to_string()
}
