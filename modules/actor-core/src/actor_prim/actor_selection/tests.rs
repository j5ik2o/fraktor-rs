//! Tests for ActorSelectionResolver

#[cfg(test)]
mod tests {
  use crate::actor_prim::{
    actor_path::{ActorPath, ActorPathError},
    actor_selection::ActorSelectionResolver,
  };

  #[test]
  fn test_resolve_current_path() {
    // 現在のパスを維持
    let base = ActorPath::root().child("worker");
    let resolved = ActorSelectionResolver::resolve_relative(&base, ".").unwrap();
    assert_eq!(resolved.to_relative_string(), base.to_relative_string());
  }

  #[test]
  fn test_resolve_child_path() {
    // 子パスを追加
    // ActorPath::root() は guardian "cellactor" を含む
    let base = ActorPath::root().child("user");
    let resolved = ActorSelectionResolver::resolve_relative(&base, "worker").unwrap();
    // 期待値は /cellactor/user/worker (guardian含む)
    assert_eq!(
      resolved.to_relative_string(),
      base.child("worker").to_relative_string()
    );
  }

  #[test]
  fn test_resolve_multiple_child_path() {
    // 複数の子パスを追加
    let base = ActorPath::root();
    let resolved = ActorSelectionResolver::resolve_relative(&base, "user/worker/task").unwrap();
    let expected = base.child("user").child("worker").child("task");
    assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
  }

  #[test]
  fn test_resolve_parent_path() {
    // 親パスへ遡る
    let base = ActorPath::root().child("user").child("worker");
    let resolved = ActorSelectionResolver::resolve_relative(&base, "..").unwrap();
    let expected = ActorPath::root().child("user");
    assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
  }

  #[test]
  fn test_resolve_parent_and_child() {
    // 親へ遡って別の子を追加
    let base = ActorPath::root().child("user").child("worker");
    let resolved = ActorSelectionResolver::resolve_relative(&base, "../manager").unwrap();
    let expected = ActorPath::root().child("user").child("manager");
    assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
  }

  #[test]
  fn test_escape_guardian_fails() {
    // guardian より上位へ遡ることは禁止
    let base = ActorPath::root();
    let result = ActorSelectionResolver::resolve_relative(&base, "..");
    assert!(matches!(result, Err(ActorPathError::RelativeEscape)));
  }

  #[test]
  fn test_escape_beyond_guardian_fails() {
    // 複数の .. で guardian を超えようとする
    let base = ActorPath::root().child("user");
    let result = ActorSelectionResolver::resolve_relative(&base, "../..");
    assert!(matches!(result, Err(ActorPathError::RelativeEscape)));
  }

  #[test]
  fn test_complex_relative_path() {
    // 複雑な相対パス解決
    let base = ActorPath::root().child("user").child("worker").child("subtask");
    let resolved = ActorSelectionResolver::resolve_relative(&base, "../../manager/newtask").unwrap();
    let expected = ActorPath::root().child("user").child("manager").child("newtask");
    assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
  }

  #[test]
  fn test_empty_selection_returns_base() {
    // 空の選択式は base をそのまま返す
    let base = ActorPath::root().child("user");
    let resolved = ActorSelectionResolver::resolve_relative(&base, "").unwrap();
    assert_eq!(resolved.to_relative_string(), base.to_relative_string());
  }
}
