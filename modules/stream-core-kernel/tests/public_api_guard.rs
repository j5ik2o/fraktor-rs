use std::{
  fs,
  path::{Path, PathBuf},
};

const FORBIDDEN_PUBLIC_ITEMS: &[&str] = &[
  "new_without_system",
  "collect_values",
  "StreamMailboxSelector",
  "StreamMailboxSelectorConfig",
  "StreamMailboxPolicy",
  "stream_mailbox_selector",
  "with_stream_mailbox",
  "with_mailbox_selector",
  "mailbox_selector",
];

#[test]
fn public_stream_api_does_not_expose_actor_systemless_helpers() {
  let source_files = rust_source_files(Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
  let mut violations = Vec::new();

  for source_file in source_files {
    let content = fs::read_to_string(&source_file).expect("source file should be readable");
    for (line_index, line) in content.lines().enumerate() {
      let trimmed = line.trim_start();
      if exposes_forbidden_public_item(trimmed) {
        violations.push(format!("{}:{}", source_file.display(), line_index + 1));
      }
    }
  }

  assert!(violations.is_empty(), "forbidden public stream helpers were exposed: {violations:?}");
}

fn rust_source_files(root: PathBuf) -> Vec<PathBuf> {
  let mut files = Vec::new();
  collect_rust_source_files(&root, &mut files);
  files
}

fn collect_rust_source_files(path: &Path, files: &mut Vec<PathBuf>) {
  let entries = fs::read_dir(path).expect("source directory should be readable");
  for entry in entries {
    let entry = entry.expect("source directory entry should be readable");
    let path = entry.path();
    if path.is_dir() {
      collect_rust_source_files(&path, files);
    } else if path.extension().is_some_and(|extension| extension == "rs") {
      files.push(path);
    }
  }
}

fn exposes_forbidden_public_item(line: &str) -> bool {
  line.starts_with("pub ") && FORBIDDEN_PUBLIC_ITEMS.iter().any(|item| line.contains(item))
}
