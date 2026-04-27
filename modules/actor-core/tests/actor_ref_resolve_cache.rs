use core::cell::Cell;

use fraktor_actor_core_rs::core::kernel::{
  actor::actor_path::{ActorPath, ActorPathParser},
  serialization::{ActorRefResolveCache, ActorRefResolveCacheOutcome},
};

fn remote_path(name: &str) -> ActorPath {
  ActorPathParser::parse(&format!("fraktor.tcp://remote-sys@10.0.0.1:2552/user/{name}")).expect("remote path")
}

fn temp_path(name: &str) -> ActorPath {
  ActorPath::root().child("temp").child(name)
}

#[test]
fn should_return_miss_then_hit_when_same_actor_path_is_resolved_twice() {
  let mut cache = ActorRefResolveCache::with_limits(2, 600);
  let path = remote_path("worker");
  let calls = Cell::new(0_u32);

  let first = cache.resolve(&path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });
  let second = cache.resolve(&path, |_candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(remote_path("unexpected"))
  });

  assert!(matches!(first, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == path));
  assert!(matches!(second, Ok(ActorRefResolveCacheOutcome::Hit(value)) if value == path));
  assert_eq!(calls.get(), 1);
}

#[test]
fn should_not_cache_resolver_errors() {
  let mut cache = ActorRefResolveCache::with_limits(2, 600);
  let path = remote_path("unstable");
  let calls = Cell::new(0_u32);

  let first = cache.resolve(&path, |_candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Err::<ActorPath, &'static str>("resolve failed")
  });
  let second = cache.resolve(&path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });

  assert!(matches!(first, Err("resolve failed")));
  assert!(matches!(second, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == path));
  assert_eq!(calls.get(), 2);
}

#[test]
fn should_not_cache_temp_actor_paths() {
  let mut cache = ActorRefResolveCache::with_limits(2, 600);
  let path = temp_path("reply");
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

#[test]
fn should_cache_regular_actor_path_when_temp_is_only_actor_name() {
  let mut cache = ActorRefResolveCache::with_limits(2, 600);
  let path = ActorPath::root().child("parent").child("temp");
  let calls = Cell::new(0_u32);

  let first = cache.resolve(&path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });
  let second = cache.resolve(&path, |_candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(remote_path("unexpected"))
  });

  assert!(matches!(first, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == path));
  assert!(matches!(second, Ok(ActorRefResolveCacheOutcome::Hit(value)) if value == path));
  assert_eq!(calls.get(), 1);
}

#[test]
fn should_evict_oldest_entry_when_capacity_is_exceeded() {
  let mut cache = ActorRefResolveCache::with_limits(2, 600);
  let first_path = remote_path("one");
  let second_path = remote_path("two");
  let third_path = remote_path("three");
  let calls = Cell::new(0_u32);

  cache
    .resolve(&first_path, |candidate: &ActorPath| {
      calls.set(calls.get() + 1);
      Ok::<ActorPath, &'static str>(candidate.clone())
    })
    .expect("first resolve");
  cache
    .resolve(&second_path, |candidate: &ActorPath| {
      calls.set(calls.get() + 1);
      Ok::<ActorPath, &'static str>(candidate.clone())
    })
    .expect("second resolve");
  cache
    .resolve(&third_path, |candidate: &ActorPath| {
      calls.set(calls.get() + 1);
      Ok::<ActorPath, &'static str>(candidate.clone())
    })
    .expect("third resolve");

  let resolved_again = cache.resolve(&first_path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });

  assert!(matches!(resolved_again, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == first_path));
  assert_eq!(calls.get(), 4);
}

#[test]
fn should_reject_zero_capacity() {
  let result = std::panic::catch_unwind(|| ActorRefResolveCache::<ActorPath>::with_limits(0, 600));

  assert!(result.is_err(), "zero capacity must be rejected at construction time");
}

#[test]
fn should_use_default_capacity_and_threshold_when_default_is_called() {
  let mut cache: ActorRefResolveCache<ActorPath> = ActorRefResolveCache::default();
  let path = remote_path("default");

  let outcome = cache.resolve(&path, |candidate: &ActorPath| Ok::<ActorPath, &'static str>(candidate.clone()));

  assert!(matches!(outcome, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == path));
}

#[test]
fn should_evict_least_recently_used_entry_when_first_inserted_is_more_recent() {
  let mut cache = ActorRefResolveCache::with_limits(2, 600);
  let first_path = remote_path("first");
  let second_path = remote_path("second");
  let third_path = remote_path("third");

  cache
    .resolve(&first_path, |candidate: &ActorPath| Ok::<ActorPath, &'static str>(candidate.clone()))
    .expect("first miss");
  cache
    .resolve(&second_path, |candidate: &ActorPath| Ok::<ActorPath, &'static str>(candidate.clone()))
    .expect("second miss");
  cache
    .resolve(&first_path, |candidate: &ActorPath| Ok::<ActorPath, &'static str>(candidate.clone()))
    .expect("first hit");
  cache
    .resolve(&third_path, |candidate: &ActorPath| Ok::<ActorPath, &'static str>(candidate.clone()))
    .expect("third miss evicts second");

  let first_again =
    cache.resolve(&first_path, |candidate: &ActorPath| Ok::<ActorPath, &'static str>(candidate.clone()));
  let second_again =
    cache.resolve(&second_path, |candidate: &ActorPath| Ok::<ActorPath, &'static str>(candidate.clone()));

  assert!(matches!(first_again, Ok(ActorRefResolveCacheOutcome::Hit(value)) if value == first_path));
  assert!(matches!(second_again, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == second_path));
}

#[test]
fn should_evict_stale_entry_before_least_recently_used_entry() {
  let mut cache = ActorRefResolveCache::with_limits(2, 1);
  let first_path = remote_path("one");
  let second_path = remote_path("two");
  let third_path = remote_path("three");
  let calls = Cell::new(0_u32);

  cache
    .resolve(&first_path, |candidate: &ActorPath| {
      calls.set(calls.get() + 1);
      Ok::<ActorPath, &'static str>(candidate.clone())
    })
    .expect("first resolve");
  cache
    .resolve(&second_path, |candidate: &ActorPath| {
      calls.set(calls.get() + 1);
      Ok::<ActorPath, &'static str>(candidate.clone())
    })
    .expect("second resolve");
  cache
    .resolve(&first_path, |_candidate: &ActorPath| {
      calls.set(calls.get() + 1);
      Ok::<ActorPath, &'static str>(remote_path("unexpected-first"))
    })
    .expect("first hit");
  cache
    .resolve(&third_path, |candidate: &ActorPath| {
      calls.set(calls.get() + 1);
      Ok::<ActorPath, &'static str>(candidate.clone())
    })
    .expect("third resolve");

  let second_again = cache.resolve(&second_path, |_candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(remote_path("unexpected-second"))
  });
  let first_again = cache.resolve(&first_path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });

  assert!(matches!(second_again, Ok(ActorRefResolveCacheOutcome::Hit(value)) if value == second_path));
  assert!(matches!(first_again, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == first_path));
  assert_eq!(calls.get(), 4);
}
