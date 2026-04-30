use std::{
  fs,
  path::{Path, PathBuf},
};

const STREAM_EXAMPLES: &[&str] = &[
  "stream/first-example/main.rs",
  "stream/basics/main.rs",
  "stream/graphs/main.rs",
  "stream/composition/main.rs",
  "stream/rate/main.rs",
];

const FORBIDDEN_DIRECT_EXECUTION_HELPERS: &[&str] =
  &["run_with_collect_sink", "collect_values", "new_without_system", "ActorMaterializer::new_without_system"];

#[test]
fn stream_showcases_run_through_actor_materializer_and_sink() {
  for example in stream_examples() {
    let content = fs::read_to_string(&example).expect("stream showcase should be readable");

    assert!(
      content.contains("support::start_materializer()"),
      "stream showcase must start ActorSystem-backed materializer: {}",
      example.display()
    );
    assert!(
      content.contains(".into_mat(Sink::"),
      "stream showcase must connect the graph to a Sink: {}",
      example.display()
    );
    assert!(
      content.contains(".run(&mut materializer)"),
      "stream showcase must run through ActorMaterializer: {}",
      example.display()
    );
  }
}

#[test]
fn stream_showcases_do_not_use_actor_systemless_execution_helpers() {
  for example in stream_examples() {
    let content = fs::read_to_string(&example).expect("stream showcase should be readable");

    for forbidden in FORBIDDEN_DIRECT_EXECUTION_HELPERS {
      assert!(
        !content.contains(forbidden),
        "stream showcase must not use actor-systemless helper `{forbidden}`: {}",
        example.display()
      );
    }
  }
}

fn stream_examples() -> Vec<PathBuf> {
  STREAM_EXAMPLES.iter().map(|relative_path| manifest_dir().join(relative_path)).collect()
}

fn manifest_dir() -> &'static Path {
  Path::new(env!("CARGO_MANIFEST_DIR"))
}
