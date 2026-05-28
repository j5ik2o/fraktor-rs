use std::{
  env, fs,
  path::{Path, PathBuf},
  process::{Command, Output},
  time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn assert_fixture_build_failure_contains(name: &str, source: &str, expected: &str) {
  let crate_dir = unique_crate_dir(name);
  write_fixture_crate(&crate_dir, name, source);
  copy_workspace_cargo_context(&crate_dir);

  let output = match Command::new("cargo")
    .arg("check")
    .arg("--offline")
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

fn copy_workspace_cargo_context(crate_dir: &Path) {
  let workspace_root = workspace_root();
  copy_required_file(&workspace_root.join("Cargo.lock"), &crate_dir.join("Cargo.lock"));
  copy_required_file(&workspace_root.join("rust-toolchain.toml"), &crate_dir.join("rust-toolchain.toml"));

  let workspace_cargo_config = workspace_root.join(".cargo").join("config.toml");
  if workspace_cargo_config.exists() {
    let fixture_cargo_dir = crate_dir.join(".cargo");
    if let Err(error) = fs::create_dir_all(&fixture_cargo_dir) {
      panic!("fixture .cargo directory should be created: {error}");
    }
    copy_required_file(&workspace_cargo_config, &fixture_cargo_dir.join("config.toml"));
  }
}

fn workspace_root() -> PathBuf {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  match manifest_dir.ancestors().find(|path| path.join("Cargo.lock").is_file()) {
    | Some(path) => path.to_path_buf(),
    | None => panic!("workspace root with Cargo.lock should be discoverable from {}", manifest_dir.display()),
  }
}

fn copy_required_file(from: &Path, to: &Path) {
  if let Err(error) = fs::copy(from, to) {
    panic!("{} should be copied to {}: {error}", from.display(), to.display());
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
