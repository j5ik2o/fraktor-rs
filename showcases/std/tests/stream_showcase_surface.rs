use std::{
  fs,
  path::{Path, PathBuf},
};

const FORBIDDEN_DIRECT_EXECUTION_HELPERS: &[&str] =
  &["run_with_collect_sink", "collect_values", "new_without_system", "ActorMaterializer::new_without_system"];

#[test]
fn stream_showcases_do_not_use_forbidden_helpers() {
  let examples = stream_examples();
  assert!(
    !examples.is_empty(),
    "expected at least one stream showcase under {}",
    manifest_dir().join("stream").display()
  );
  for example in examples {
    let content = fs::read_to_string(&example).expect("stream showcase should be readable");

    for forbidden in FORBIDDEN_DIRECT_EXECUTION_HELPERS {
      assert!(
        !content.contains(forbidden),
        "stream showcase must not use actor-systemless helper `{forbidden}`: {}",
        example.display()
      );
    }

    assert!(
      !content.contains("support::"),
      "stream showcase must demonstrate direct public API usage instead of support helpers: {}",
      example.display()
    );
  }
}

fn stream_examples() -> Vec<PathBuf> {
  let mut examples = Vec::new();
  collect_stream_examples(&manifest_dir().join("stream"), &mut examples);
  examples.sort();
  examples
}

fn collect_stream_examples(directory: &Path, out: &mut Vec<PathBuf>) {
  for entry in fs::read_dir(directory).expect("stream showcase directory should be readable") {
    let entry = entry.expect("directory entry should be readable");
    let path = entry.path();
    if path.is_dir() {
      collect_stream_examples(&path, out);
      continue;
    }
    if path.file_name().and_then(|file_name| file_name.to_str()) == Some("main.rs") {
      out.push(path);
    }
  }
}

fn manifest_dir() -> &'static Path {
  Path::new(env!("CARGO_MANIFEST_DIR"))
}
