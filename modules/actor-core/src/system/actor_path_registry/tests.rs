//! Tests for ActorPathRegistry

#[cfg(test)]
mod tests {
  use alloc::format;

  use crate::{
    actor_prim::{actor_path::ActorPath, Pid},
    system::actor_path_registry::ActorPathRegistry,
  };

  #[test]
  fn test_register_and_retrieve() {
    // PIDとパスを登録し、取得できることを確認
    let mut registry = ActorPathRegistry::new();
    let pid = Pid::new(1, 0);
    let path = ActorPath::root().child("user").child("worker");

    registry.register(pid, &path);

    let handle = registry.get(&pid).expect("handle should exist");
    assert_eq!(handle.pid(), pid);
    assert_eq!(handle.canonical_uri(), path.to_canonical_uri());
  }

  #[test]
  fn test_unregister() {
    // 登録後に削除できることを確認
    let mut registry = ActorPathRegistry::new();
    let pid = Pid::new(1, 0);
    let path = ActorPath::root().child("user");

    registry.register(pid, &path);
    assert!(registry.get(&pid).is_some());

    registry.unregister(&pid);
    assert!(registry.get(&pid).is_none());
  }

  #[test]
  fn test_canonical_uri() {
    // canonical_uri ヘルパーが正しく動作することを確認
    let mut registry = ActorPathRegistry::new();
    let pid = Pid::new(1, 0);
    let path = ActorPath::root().child("user").child("manager");

    registry.register(pid, &path);

    let uri = registry.canonical_uri(&pid).expect("URI should exist");
    assert_eq!(uri, path.to_canonical_uri());
  }

  #[test]
  fn test_nonexistent_pid() {
    // 存在しないPIDに対してはNoneを返すことを確認
    let registry = ActorPathRegistry::new();
    let pid = Pid::new(999, 0);

    assert!(registry.get(&pid).is_none());
    assert!(registry.canonical_uri(&pid).is_none());
  }

  #[test]
  fn test_multiple_registrations() {
    // 複数のPIDを登録できることを確認
    let mut registry = ActorPathRegistry::new();

    for i in 0..10 {
      let pid = Pid::new(i, 0);
      let path = ActorPath::root().child(&format!("worker-{}", i));
      registry.register(pid, &path);
    }

    for i in 0..10 {
      let pid = Pid::new(i, 0);
      let handle = registry.get(&pid).expect("handle should exist");
      assert_eq!(handle.pid(), pid);
    }
  }
}
