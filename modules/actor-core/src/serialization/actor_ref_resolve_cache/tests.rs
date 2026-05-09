use core::cell::Cell;

use crate::{
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
fn evict_picks_oldest_stale_entry_when_multiple_candidates_exist() {
  // Vec 挿入順だけで stale entry を選ぶと、 age threshold を辛うじて越えた直近アクセスの
  // entry が、 もっと古い未アクセス entry より先に evict され LRU 意図を崩す。 stale 候補の中で
  // accessed_at が最小の entry が選ばれるかを検証する。
  fn echo_resolver(candidate: &ActorPath) -> Result<ActorPath, &'static str> {
    Ok(candidate.clone())
  }

  let mut cache = ActorRefResolveCache::with_limits(3, 2);
  let path_a = remote_path("a");
  let path_b = remote_path("b");
  let path_c = remote_path("c");
  let path_d = remote_path("d");

  // 初期 3 entry を挿入。 accessed_at は A=1, B=2, C=3。
  cache.resolve(&path_a, echo_resolver).expect("insert a");
  cache.resolve(&path_b, echo_resolver).expect("insert b");
  cache.resolve(&path_c, echo_resolver).expect("insert c");
  // A と B を再アクセス。 accessed_at は A=4, B=5, C=3 となり、
  // Vec の挿入順は [A, B, C] のままだが LRU 順序は C → A → B。
  cache.resolve(&path_a, echo_resolver).expect("touch a");
  cache.resolve(&path_b, echo_resolver).expect("touch b");

  // 4 個目を挿入。 epoch=6, threshold=2 で A と C が stale (B は not stale)。
  // 旧実装は Vec 順序で先頭 stale = A を evict してしまうが、 LRU 意図に沿って
  // accessed_at が最小の C を evict すべき。
  cache.resolve(&path_d, echo_resolver).expect("insert d");

  let resolved_a = cache.resolve(&path_a, echo_resolver).expect("resolve a");
  let resolved_c = cache.resolve(&path_c, echo_resolver).expect("resolve c");
  assert!(
    matches!(resolved_a, ActorRefResolveCacheOutcome::Hit(ref value) if *value == path_a),
    "A should remain cached because C is the older stale entry; got {resolved_a:?}",
  );
  assert!(
    matches!(resolved_c, ActorRefResolveCacheOutcome::Miss(ref value) if *value == path_c),
    "C should have been evicted as the oldest stale entry; got {resolved_c:?}",
  );
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
