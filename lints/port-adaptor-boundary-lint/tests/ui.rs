use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

#[test]
fn ui() {
  let mut test = dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "tests/ui");
  test.dylint_toml(dylint_toml_content());
  test.run();
}

fn dylint_toml_content() -> String {
  let path = library_path();
  format!("[[dylint]]\nname = '{}'\npath = '{}'\n", env!("CARGO_PKG_NAME"), path.display())
}

fn library_path() -> PathBuf {
  let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let toolchain = resolve_toolchain(&manifest_dir);
  let target_root = manifest_dir.join("target");
  let target_dir = target_root.join("debug");
  let crate_name = env!("CARGO_PKG_NAME").replace('-', "_");
  let plain_name = format!("{}{}{}", std::env::consts::DLL_PREFIX, crate_name, std::env::consts::DLL_SUFFIX);
  let plain_path = target_dir.join(&plain_name);

  assert!(
    Command::new("rustup")
      .args(["run", toolchain.as_str(), "cargo", "build"])
      .env("CARGO_TARGET_DIR", &target_root)
      .current_dir(&manifest_dir)
      .status()
      .expect("cargo build failed")
      .success(),
    "cargo build failed"
  );

  let toolchain_name = format!(
    "{}@{}{}",
    plain_name.trim_end_matches(std::env::consts::DLL_SUFFIX),
    toolchain,
    std::env::consts::DLL_SUFFIX
  );
  let toolchain_path = target_dir.join(toolchain_name);
  fs::create_dir_all(&target_dir).expect("failed to ensure target directory");
  fs::copy(&plain_path, &toolchain_path).expect("failed to copy lint library");
  toolchain_path
}

fn resolve_toolchain(manifest_dir: &Path) -> String {
  std::env::var("RUSTUP_TOOLCHAIN")
    .ok()
    .filter(|toolchain| !toolchain.is_empty())
    .or_else(|| toolchain_channel_from_file(&manifest_dir.join("rust-toolchain.toml")))
    .or_else(active_toolchain)
    .unwrap_or_else(|| "nightly".to_string())
}

fn toolchain_channel_from_file(path: &Path) -> Option<String> {
  let content = fs::read_to_string(path).ok()?;
  content.lines().find_map(|line| {
    let line = line.trim();
    line
      .strip_prefix("channel")
      .and_then(|line| line.split_once('='))
      .and_then(|(_, value)| value.trim().trim_matches('"').split_whitespace().next())
      .map(ToString::to_string)
  })
}

fn active_toolchain() -> Option<String> {
  let output = Command::new("rustup").args(["show", "active-toolchain"]).output().ok()?;
  if !output.status.success() {
    return None;
  }

  String::from_utf8(output.stdout)
    .ok()?
    .split_whitespace()
    .next()
    .map(ToString::to_string)
}
