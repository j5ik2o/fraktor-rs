use core::cell::Cell;

use fraktor_actor_core_rs::{
  actor::actor_path::{ActorPath, ActorPathParser, GuardianKind},
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
fn should_cache_paths_under_system_guardian() {
  let mut cache = ActorRefResolveCache::with_limits(2, 600);
  let path = ActorPath::root_with_guardian(GuardianKind::System).child("temp").child("entry");
  let calls = Cell::new(0_u32);

  let first = cache.resolve(&path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });
  let second = cache.resolve(&path, |_candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(remote_path("unexpected"))
  });

  // システム guardian 配下のパスは一時 actor 規約の対象外なので cache 対象。
  assert!(matches!(first, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == path));
  assert!(matches!(second, Ok(ActorRefResolveCacheOutcome::Hit(value)) if value == path));
  assert_eq!(calls.get(), 1);
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
  // 旧シナリオは threshold=1 で両 entry が stale になり、 Vec 挿入順 (`position`) で先頭を
  // evict するロジックを「first が stale なら evict される」と擬制していた。 stale 中で最古
  // (accessed_at 最小) を選ぶ正しい LRU 意図に合わせ、 second だけが stale な状態を作って
  // 「stale な second が non-stale な first より先に evict される」を検証する。
  let mut cache = ActorRefResolveCache::with_limits(2, 3);
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
  // first を 2 回 touch して accessed_at を進める一方、 second は放置する。
  // threshold=3 で second だけが age を超え、 first は超えない構成にする。
  cache
    .resolve(&first_path, |_candidate: &ActorPath| {
      calls.set(calls.get() + 1);
      Ok::<ActorPath, &'static str>(remote_path("unexpected-first"))
    })
    .expect("first hit");
  cache
    .resolve(&first_path, |_candidate: &ActorPath| {
      calls.set(calls.get() + 1);
      Ok::<ActorPath, &'static str>(remote_path("unexpected-first"))
    })
    .expect("first hit again");
  cache
    .resolve(&third_path, |candidate: &ActorPath| {
      calls.set(calls.get() + 1);
      Ok::<ActorPath, &'static str>(candidate.clone())
    })
    .expect("third resolve");

  // first を先に確認して Hit させる (first の accessed_at を更新)。 second は evict された
  // 想定のため、 resolve 時に再挿入される。 先に second を呼ぶと再挿入の時点で別の evict
  // (LRU で first が選ばれる) が走るので、 順序は first → second の固定とする。
  let first_again = cache.resolve(&first_path, |_candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(remote_path("unexpected-first"))
  });
  let second_again = cache.resolve(&second_path, |candidate: &ActorPath| {
    calls.set(calls.get() + 1);
    Ok::<ActorPath, &'static str>(candidate.clone())
  });

  assert!(matches!(first_again, Ok(ActorRefResolveCacheOutcome::Hit(value)) if value == first_path));
  assert!(matches!(second_again, Ok(ActorRefResolveCacheOutcome::Miss(value)) if value == second_path));
  assert_eq!(calls.get(), 4);
}
