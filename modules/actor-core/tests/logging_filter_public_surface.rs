#![cfg(not(target_os = "none"))]

use std::{
  env, fs,
  path::{Path, PathBuf},
  process::{Command, Output},
  time::{SystemTime, UNIX_EPOCH},
};

const LOGGING_FILTER_SOURCE: &str = r#"use fraktor_actor_core_rs::{
  event::logging::{DefaultLoggingFilter, LogEvent, LogLevel, LoggingFilter},
  system::state::{SystemStateShared, system_state::SystemState},
};

struct MarkerOnlyFilter;

impl LoggingFilter for MarkerOnlyFilter {
  fn should_publish(&self, event: &LogEvent) -> bool {
    event.marker_name() == Some("pekkoDeadLetter")
  }
}

fn main() {
  let mut state = SystemState::new();
  state.set_logging_filter(DefaultLoggingFilter::new(LogLevel::Error));
  state.set_logging_filter(MarkerOnlyFilter);
  let shared = SystemStateShared::new(state);
  shared.set_logging_filter(DefaultLoggingFilter::new(LogLevel::Error));
  shared.set_logging_filter(MarkerOnlyFilter);
}
"#;

#[test]
fn logging_filter_contract_compiles_from_external_crate() {
  assert_fixture_build("logging-filter-public-surface", LOGGING_FILTER_SOURCE);
}

fn assert_fixture_build(name: &str, source: &str) {
  let crate_dir = unique_crate_dir(name);
  write_fixture_crate(&crate_dir, name, source);

  let output = match Command::new("cargo")
    .arg("check")
    .arg("--quiet")
    .env("CARGO_TARGET_DIR", crate_dir.join("target"))
    .current_dir(&crate_dir)
    .output()
  {
    | Ok(output) => output,
    | Err(error) => panic!("fixture cargo check should start: {error}"),
  };

  let rendered = render_output(&output);
  let cleanup_result = fs::remove_dir_all(&crate_dir);

  assert!(output.status.success(), "fixture should compile successfully:\n{rendered}");

  if let Err(error) = cleanup_result {
    panic!("fixture directory cleanup should succeed: {error}");
  }
}

fn write_fixture_crate(crate_dir: &Path, name: &str, source: &str) {
  let src_dir = crate_dir.join("src");
  if let Err(error) = fs::create_dir_all(&src_dir) {
    panic!("fixture src directory should be created: {error}");
  }
  if let Err(error) = fs::write(crate_dir.join("Cargo.toml"), fixture_manifest(name)) {
    panic!("fixture manifest should be written: {error}");
  }
  if let Err(error) = fs::write(src_dir.join("main.rs"), source) {
    panic!("fixture main source should be written: {error}");
  }
}

fn fixture_manifest(name: &str) -> String {
  let manifest_dir = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
  format!(
    r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
fraktor-actor-core-rs = {{ path = "{manifest_dir}" }}
"#
  )
}

fn unique_crate_dir(name: &str) -> PathBuf {
  let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
    | Ok(duration) => duration.as_nanos(),
    | Err(error) => panic!("system clock should be after unix epoch: {error}"),
  };
  let dir = env::temp_dir().join(format!("fraktor-actor-core-rs-{name}-{}-{timestamp}", std::process::id()));
  if let Err(error) = fs::create_dir_all(&dir) {
    panic!("unique crate directory should be created: {error}");
  }
  dir
}

fn render_output(output: &Output) -> String {
  let stdout = String::from_utf8_lossy(&output.stdout);
  let stderr = String::from_utf8_lossy(&output.stderr);
  format!("status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}", output.status.code())
}
