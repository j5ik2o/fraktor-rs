#![cfg(not(target_os = "none"))]

use std::{
  env, fs,
  path::{Path, PathBuf},
  process::{Command, Output},
  time::{SystemTime, UNIX_EPOCH},
};

use fraktor_persistence_core_typed_rs::{DurableStateSignal, PersistenceEffectorSignal};

const FORGED_SIGNAL_SOURCE: &str = r#"
use fraktor_persistence_core_typed_rs::DurableStateSignal;

fn main() {
  let _signal: DurableStateSignal<u64> = DurableStateSignal::RecoveryCompleted {
    auth: Default::default(),
    state: None,
    revision: 0,
  };
}
"#;

const REUSED_AUTH_SOURCE: &str = r#"
use fraktor_persistence_core_typed_rs::DurableStateSignal;

fn forge(signal: DurableStateSignal<u64>) -> DurableStateSignal<u64> {
  let auth = match signal {
    | DurableStateSignal::RecoveryCompleted { auth, .. }
    | DurableStateSignal::RecoveryFailed { auth, .. }
    | DurableStateSignal::StatePersisted { auth, .. }
    | DurableStateSignal::StateDeleted { auth, .. }
    | DurableStateSignal::PersistenceFailed { auth, .. } => auth,
  };

  DurableStateSignal::RecoveryCompleted {
    auth,
    state: Some(999),
    revision: 999,
  }
}

fn main() {}
"#;

#[derive(Clone, Debug)]
enum PrivateMessage {
  Durable(Option<DurableStateSignal<u32>>),
}

#[test]
fn durable_state_signal_can_be_wrapped_by_user_private_message() {
  let message = PrivateMessage::Durable(None);

  match message {
    | PrivateMessage::Durable(signal) => assert!(signal.is_none()),
  }
}

#[test]
fn durable_state_persisted_signal_is_separate_from_event_sourced_signal() {
  let durable: Option<DurableStateSignal<u32>> = None;
  let event_sourced: Option<PersistenceEffectorSignal<u32, u32>> = None;

  assert!(durable.is_none());
  assert!(event_sourced.is_none());
}

#[test]
fn durable_state_signal_auth_cannot_be_forged_from_external_crate() {
  assert_fixture_build_failure_contains("durable-state-signal-auth-forged", FORGED_SIGNAL_SOURCE, "E0277");
  assert_fixture_build_failure_contains("durable-state-signal-auth-reused", REUSED_AUTH_SOURCE, "E0639");
}

fn assert_fixture_build_failure_contains(name: &str, source: &str, expected: &str) {
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

  assert!(!output.status.success(), "fixture should fail to compile:\n{rendered}");
  assert!(rendered.contains(expected), "fixture should fail because of `{expected}`:\n{rendered}");

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
fraktor-persistence-core-typed-rs = {{ path = "{manifest_dir}" }}
"#
  )
}

fn unique_crate_dir(name: &str) -> PathBuf {
  let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
    | Ok(duration) => duration.as_nanos(),
    | Err(error) => panic!("system clock should be after unix epoch: {error}"),
  };
  let dir =
    env::temp_dir().join(format!("fraktor-persistence-core-typed-rs-{name}-{}-{timestamp}", std::process::id()));
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
