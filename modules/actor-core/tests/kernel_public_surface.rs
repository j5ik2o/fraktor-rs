#![cfg(not(target_os = "none"))]

use std::{
  env, fs,
  path::{Path, PathBuf},
  process::{Command, Output},
  time::{SystemTime, UNIX_EPOCH},
};

const PUBLIC_API_SOURCE: &str = include_str!("fixtures/kernel_public_surface/public_api.rs");
const ACTOR_CELL_STATE_SOURCE: &str = include_str!("fixtures/kernel_public_surface/actor_cell_state.rs");
const ACTOR_CELL_STATE_SHARED_SOURCE: &str = include_str!("fixtures/kernel_public_surface/actor_cell_state_shared.rs");
const RECEIVE_TIMEOUT_STATE_SOURCE: &str = include_str!("fixtures/kernel_public_surface/receive_timeout_state.rs");
const RECEIVE_TIMEOUT_STATE_SHARED_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/receive_timeout_state_shared.rs");
const CONTEXT_PIPE_WAKER_HANDLE_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/context_pipe_waker_handle.rs");
const CONTEXT_PIPE_WAKER_HANDLE_SHARED_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/context_pipe_waker_handle_shared.rs");
const SYSTEM_STATE_SHARED_INNER_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/system_state_shared_inner.rs");
const ACTOR_SYSTEM_STOP_ACTOR_SOURCE: &str = include_str!("fixtures/kernel_public_surface/actor_system_stop_actor.rs");
const ACTOR_SYSTEM_SPAWN_DETACHED_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/actor_system_spawn_detached.rs");
const ACTOR_CONTEXT_CURRENT_MESSAGE_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/actor_context_current_message.rs");
const ACTOR_CELL_STASH_PRIMITIVE_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/actor_cell_stash_primitive.rs");
const ACTOR_CELL_ADAPTER_PRIMITIVE_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/actor_cell_adapter_primitive.rs");
const MESSAGE_ADAPTER_INTERNAL_TYPES_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/message_adapter_internal_types.rs");
const TYPED_UNHANDLED_MESSAGE_EVENT_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/typed_unhandled_message_event.rs");
const ROUTING_CONSISTENT_HASHING_LOGIC_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/routing_consistent_hashing_logic.rs");
const ROUTING_SMALLEST_MAILBOX_LOGIC_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/routing_smallest_mailbox_logic.rs");
const ROUTING_ROUTER_FROM_CONFIG_SOURCE: &str =
  include_str!("fixtures/kernel_public_surface/routing_router_from_config.rs");

#[test]
fn official_kernel_public_api_compiles_from_external_crate() {
  // 公式公開 API がクレート外からコンパイルできることを保証する
  assert_fixture_build("kernel-public-api", PUBLIC_API_SOURCE, true);
}

#[test]
fn internal_actor_helpers_are_not_reachable_from_external_crate() {
  let fixtures = [
    ("kernel-actor-cell-state", ACTOR_CELL_STATE_SOURCE, "ActorCellState"),
    ("kernel-actor-cell-state-shared", ACTOR_CELL_STATE_SHARED_SOURCE, "ActorCellStateShared"),
    ("kernel-receive-timeout-state", RECEIVE_TIMEOUT_STATE_SOURCE, "ReceiveTimeoutState"),
    ("kernel-receive-timeout-state-shared", RECEIVE_TIMEOUT_STATE_SHARED_SOURCE, "ReceiveTimeoutStateShared"),
    ("kernel-context-pipe-waker-handle", CONTEXT_PIPE_WAKER_HANDLE_SOURCE, "ContextPipeWakerHandle"),
    (
      "kernel-context-pipe-waker-handle-shared",
      CONTEXT_PIPE_WAKER_HANDLE_SHARED_SOURCE,
      "ContextPipeWakerHandleShared",
    ),
    ("kernel-system-state-shared-inner", SYSTEM_STATE_SHARED_INNER_SOURCE, "inner"),
    ("kernel-actor-system-stop-actor", ACTOR_SYSTEM_STOP_ACTOR_SOURCE, "stop_actor"),
    ("kernel-actor-system-spawn-detached", ACTOR_SYSTEM_SPAWN_DETACHED_SOURCE, "spawn_detached"),
    ("kernel-actor-context-current-message", ACTOR_CONTEXT_CURRENT_MESSAGE_SOURCE, "set_current_message"),
    ("kernel-actor-cell-stash-primitive", ACTOR_CELL_STASH_PRIMITIVE_SOURCE, "unstash_messages_with_limit"),
    ("kernel-actor-cell-adapter-primitive", ACTOR_CELL_ADAPTER_PRIMITIVE_SOURCE, "acquire_adapter_handle"),
    ("kernel-message-adapter-internal-types", MESSAGE_ADAPTER_INTERNAL_TYPES_SOURCE, "MessageAdapterRegistration"),
    ("kernel-typed-unhandled-message-event", TYPED_UNHANDLED_MESSAGE_EVENT_SOURCE, "TypedUnhandledMessageEvent"),
  ];

  for (name, source, expected_symbol) in fixtures {
    assert_fixture_build_failure_contains(name, source, expected_symbol);
  }
}

#[test]
fn public_routing_types_compile_from_external_crate() {
  // ConsistentHashingRoutingLogic / SmallestMailboxRoutingLogic は公開 API
  let fixtures = [
    ("kernel-routing-consistent-hashing-logic", ROUTING_CONSISTENT_HASHING_LOGIC_SOURCE),
    ("kernel-routing-smallest-mailbox-logic", ROUTING_SMALLEST_MAILBOX_LOGIC_SOURCE),
  ];

  for (name, source) in fixtures {
    assert_fixture_build(name, source, true);
  }
}

#[test]
fn internal_routing_helpers_are_not_reachable_from_external_crate() {
  let fixtures = [("kernel-routing-router-from-config", ROUTING_ROUTER_FROM_CONFIG_SOURCE, "from_config")];

  for (name, source, expected_symbol) in fixtures {
    assert_fixture_build_failure_contains(name, source, expected_symbol);
  }
}

fn assert_fixture_build(name: &str, source: &str, expect_success: bool) {
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

  if expect_success {
    assert!(output.status.success(), "fixture should compile successfully:\n{rendered}");
  } else {
    assert!(!output.status.success(), "fixture should fail to compile:\n{rendered}");
  }

  if let Err(error) = cleanup_result {
    panic!("fixture directory cleanup should succeed: {error}");
  }
}

fn assert_fixture_build_failure_contains(name: &str, source: &str, expected_symbol: &str) {
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
  assert!(
    rendered.contains(expected_symbol),
    "fixture should fail because `{expected_symbol}` is not public:\n{rendered}"
  );
  assert!(
    rendered.contains("private")
      || rendered.contains("unresolved import")
      || rendered.contains("no function or associated item")
      || rendered.contains("not found"),
    "fixture should report a visibility diagnostic for `{expected_symbol}`:\n{rendered}"
  );

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
