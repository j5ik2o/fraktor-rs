use std::path::{Path, PathBuf};

use crate::core::event::stream::EventStreamEvent;

const REMOVED_STD_ALIAS_FILES: &[&str] = &[
  "src/std/dead_letter.rs",
  "src/std/error.rs",
  "src/std/futures.rs",
  "src/std/messaging.rs",
  "src/std/actor.rs",
  "src/std/dispatch.rs",
  "src/std/dispatch/mailbox.rs",
  "src/std/dispatch/dispatcher.rs",
  "src/std/event.rs",
  "src/std/event/logging.rs",
  "src/std/event/stream.rs",
  "src/std/dispatch/dispatcher/types.rs",
  "src/std/event/stream/types.rs",
  "src/std/props.rs",
  "src/std/scheduler.rs",
  "src/std/system.rs",
  "src/std/typed.rs",
  "src/std/typed/actor.rs",
  "src/std/typed/behavior.rs",
  "src/std/typed/spawn_protocol.rs",
  "src/std/typed/stash_buffer.rs",
  "src/std/typed/typed_ask_future.rs",
  "src/std/typed/typed_ask_response.rs",
];

struct NoopSubscriber;

impl crate::std::event::stream::EventStreamSubscriber for NoopSubscriber {
  fn on_event(&mut self, _event: &EventStreamEvent) {}
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
fn std_public_modules_expose_only_live_entry_points() {
  let _behaviors = core::marker::PhantomData::<crate::std::typed::Behaviors>;
  let _props = core::marker::PhantomData::<crate::std::typed::TypedProps<String>>;
  let _system = core::marker::PhantomData::<crate::std::typed::TypedActorSystem<String>>;
  let _shared = core::marker::PhantomData::<crate::std::event::stream::EventStreamSubscriberShared>;

  let _subscriber = crate::std::event::stream::subscriber_handle(NoopSubscriber);
}

fn display_relative_path(base: &Path, path: &Path) -> String {
  path.strip_prefix(base).map(PathBuf::from).unwrap_or_else(|_| path.to_path_buf()).display().to_string()
}
