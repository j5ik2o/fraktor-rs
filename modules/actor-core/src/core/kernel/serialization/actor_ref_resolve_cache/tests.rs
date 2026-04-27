use core::cell::Cell;

use crate::core::kernel::{
  actor::actor_path::{ActorPath, ActorPathParser},
  serialization::{ActorRefResolveCache, ActorRefResolveCacheOutcome},
};

fn remote_path(name: &str) -> ActorPath {
  ActorPathParser::parse(&alloc::format!("fraktor.tcp://remote-sys@10.0.0.1:2552/user/{name}")).expect("remote path")
}

#[test]
fn resolve_returns_miss_then_hit() {
  let mut cache = ActorRefResolveCache::with_limits(2, 2);
  let path = remote_path("worker");
  let calls = Cell::new(0_u32);

  let first = cache.resolve(&path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });
  let second = cache.resolve(&path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });

  assert!(matches!(first, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == path));
  assert!(matches!(second, Ok(ActorRefResolveCacheOutcome::Hit(value)) if value == path));
  assert_eq!(calls.get(), 1);
}

#[test]
fn resolve_keeps_errors_out_of_cache() {
  let mut cache = ActorRefResolveCache::with_limits(2, 2);
  let path = remote_path("worker");
  let calls = Cell::new(0_u32);

  let failed = cache.resolve(&path, |_candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Err::<ActorPath, &'static str>("failed")
  });
  let recovered = cache.resolve(&path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });

  assert_eq!(failed, Err("failed"));
  assert!(matches!(recovered, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == path));
  assert_eq!(calls.get(), 2);
}

#[test]
fn resolve_does_not_cache_temporary_paths() {
  let mut cache = ActorRefResolveCache::with_limits(2, 2);
  let path = ActorPath::root().child("temp").child("reply");
  let calls = Cell::new(0_u32);

  let first = cache.resolve(&path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });
  let second = cache.resolve(&path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });

  assert!(matches!(first, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == path));
  assert!(matches!(second, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == path));
  assert_eq!(calls.get(), 2);
}
