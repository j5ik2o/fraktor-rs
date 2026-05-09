//! Compile-time visibility assertion for dispatcher executor types.
//!
//! The test below verifies that `AffinityExecutor` and `AffinityExecutorFactory`
//! are publicly reachable from an external crate. We generate a tiny fixture
//! crate that `use`s the types and run `cargo check` on it.

#![cfg(not(target_os = "none"))]

use std::{
  env, fs,
  path::{Path, PathBuf},
  process::{Command, Output},
  sync::mpsc::{Receiver, TryRecvError, channel},
  thread,
  time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use fraktor_actor_adaptor_std_rs::std::{
  dispatch::dispatcher::{AffinityExecutorFactory, PinnedExecutorFactory},
  system::std_actor_system_config,
  tick_driver::TestTickDriver,
};
use fraktor_actor_core_rs::dispatch::dispatcher::ExecutorFactory;

const AFFINITY_EXECUTOR_SOURCE: &str = r#"use fraktor_actor_adaptor_std_rs::std::dispatch::dispatcher::AffinityExecutor;

fn main() {
  let _ = core::mem::size_of::<AffinityExecutor>();
}
"#;

const AFFINITY_EXECUTOR_FACTORY_SOURCE: &str = r#"use fraktor_actor_adaptor_std_rs::std::dispatch::dispatcher::AffinityExecutorFactory;

fn main() {
  let _ = core::mem::size_of::<AffinityExecutorFactory>();
}
"#;

const PINNED_EXECUTOR_SOURCE: &str = r#"use fraktor_actor_adaptor_std_rs::std::dispatch::dispatcher::PinnedExecutor;

fn main() {
  let _ = core::mem::size_of::<PinnedExecutor>();
}
"#;

const PINNED_EXECUTOR_FACTORY_SOURCE: &str = r#"use fraktor_actor_adaptor_std_rs::std::dispatch::dispatcher::PinnedExecutorFactory;

fn main() {
  let _ = core::mem::size_of::<PinnedExecutorFactory>();
}
"#;

#[test]
fn affinity_executor_is_reachable_from_external_crate() {
  assert_fixture_build_success("dispatcher-public-surface-affinity", AFFINITY_EXECUTOR_SOURCE);
}

#[test]
fn affinity_executor_factory_is_reachable_from_external_crate() {
  assert_fixture_build_success("dispatcher-public-surface-affinity-factory", AFFINITY_EXECUTOR_FACTORY_SOURCE);
}

#[test]
fn pinned_executor_public_surface_is_reachable_from_external_crate() {
  assert_fixture_build_success("dispatcher-public-surface-pinned", PINNED_EXECUTOR_SOURCE);
}

#[test]
fn pinned_executor_factory_public_surface_is_reachable_from_external_crate() {
  assert_fixture_build_success("dispatcher-public-surface-pinned-factory", PINNED_EXECUTOR_FACTORY_SOURCE);
}

#[test]
fn std_dispatcher_factories_create_executors_and_accept_tasks() {
  // Pekko dispatcher contract: std adaptor factories must bridge into the
  // core ExecutorFactory surface and accept submitted mailbox work.
  let pinned_factory = PinnedExecutorFactory::new("pinned-contract");
  let pinned = pinned_factory.create("default-dispatcher");
  let (tx, rx) = channel();
  pinned.execute(Box::new(move || tx.send("pinned").expect("send pinned marker")), 0).expect("execute pinned task");
  assert_eq!(recv_by_yielding(&rx, "pinned task"), "pinned");
  pinned.shutdown();

  let affinity_factory = AffinityExecutorFactory::new("affinity-contract", 2, 8);
  let affinity = affinity_factory.create("default-dispatcher");
  let (tx, rx) = channel();
  affinity
    .execute(Box::new(move || tx.send("affinity").expect("send affinity marker")), 1)
    .expect("execute affinity task");
  assert_eq!(recv_by_yielding(&rx, "affinity task"), "affinity");
  affinity.shutdown();
}

#[test]
fn std_actor_system_config_installs_mailbox_clock() {
  // MB-M1 / Pekko Mailbox.scala throughputDeadlineTime: std production config
  // must install a monotonic mailbox clock before the core system state is built.
  let config = std_actor_system_config(TestTickDriver::default());
  assert!(config.mailbox_clock().is_some());
}

fn assert_fixture_build_success(name: &str, source: &str) {
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
fraktor-actor-adaptor-std-rs = {{ path = "{manifest_dir}" }}
"#
  )
}

fn unique_crate_dir(name: &str) -> PathBuf {
  let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
    | Ok(duration) => duration.as_nanos(),
    | Err(error) => panic!("system clock should be after unix epoch: {error}"),
  };
  let dir = env::temp_dir().join(format!("fraktor-actor-adaptor-std-rs-{name}-{}-{timestamp}", std::process::id()));
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

fn recv_by_yielding(rx: &Receiver<&'static str>, label: &str) -> &'static str {
  // A one-second wall-clock deadline avoids fixed-spin flakes on loaded CI while
  // still surfacing a stuck executor promptly.
  let deadline = Instant::now() + Duration::from_secs(1);
  while Instant::now() < deadline {
    match rx.try_recv() {
      | Ok(value) => return value,
      | Err(TryRecvError::Empty) => thread::yield_now(),
      | Err(TryRecvError::Disconnected) => panic!("{label} sender disconnected"),
    }
  }
  panic!("{label} did not complete");
}
