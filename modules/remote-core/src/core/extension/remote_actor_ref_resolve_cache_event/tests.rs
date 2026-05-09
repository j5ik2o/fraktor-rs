use fraktor_actor_core_rs::actor::actor_path::ActorPathParser;

use crate::core::extension::{RemoteActorRefResolveCacheEvent, RemoteActorRefResolveCacheOutcome};

#[test]
fn event_exposes_path_and_outcome() {
  let path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let event = RemoteActorRefResolveCacheEvent::new(path.clone(), RemoteActorRefResolveCacheOutcome::Miss);

  assert_eq!(event.path(), &path);
  assert_eq!(event.outcome(), RemoteActorRefResolveCacheOutcome::Miss);
}

#[test]
fn event_clone_preserves_fields() {
  let path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let event = RemoteActorRefResolveCacheEvent::new(path.clone(), RemoteActorRefResolveCacheOutcome::Hit);

  let cloned = event.clone();

  assert_eq!(cloned.path(), &path);
  assert_eq!(cloned.outcome(), RemoteActorRefResolveCacheOutcome::Hit);
}
