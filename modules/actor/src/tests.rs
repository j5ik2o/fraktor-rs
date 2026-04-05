use std::path::Path;

#[test]
fn deleted_std_tree_stays_deleted() {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let removed_paths = ["src/std.rs", "src/std"];

  for relative_path in removed_paths {
    let path = manifest_dir.join(relative_path);
    assert!(!path.exists(), "actor crate に削除済み std ツリーが復活しています: {}", path.display());
  }
}
