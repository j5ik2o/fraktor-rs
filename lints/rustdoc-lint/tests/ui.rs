use std::{fs, path::PathBuf, process::Command};

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
  let target_dir = manifest_dir.join("target").join("debug");
  let crate_name = env!("CARGO_PKG_NAME").replace('-', "_");
  let plain_name = format!(
    "{}{}{}",
    std::env::consts::DLL_PREFIX,
    crate_name,
    std::env::consts::DLL_SUFFIX
  );
  let plain_path = target_dir.join(&plain_name);

  assert!(
    Command::new("cargo")
      .args(["build"])
      .current_dir(&manifest_dir)
      .status()
      .expect("cargo build failed")
      .success(),
    "cargo build failed"
  );

  let toolchain = std::env::var("RUSTUP_TOOLCHAIN").expect("missing RUSTUP_TOOLCHAIN");
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
