use std::{error::Error, fs, path::PathBuf, process::Command};

#[test]
fn ui() -> Result<(), Box<dyn Error>> {
  let mut test = dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "tests/ui");
  test.dylint_toml(dylint_toml_content()?);
  test.run();
  Ok(())
}

fn dylint_toml_content() -> Result<String, Box<dyn Error>> {
  let path = library_path()?;
  Ok(format!("[[dylint]]\nname = '{}'\npath = '{}'\n", env!("CARGO_PKG_NAME"), path.display()))
}

fn library_path() -> Result<PathBuf, Box<dyn Error>> {
  let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let target_dir =
    std::env::var("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|_| manifest_dir.join("target")).join("debug");
  let crate_name = env!("CARGO_PKG_NAME").replace('-', "_");
  let plain_name = format!("{}{}{}", std::env::consts::DLL_PREFIX, crate_name, std::env::consts::DLL_SUFFIX);
  let plain_path = target_dir.join(&plain_name);

  let mut command = Command::new("cargo");
  command.args(["build"]).current_dir(&manifest_dir);
  if let Ok(target_dir_env) = std::env::var("CARGO_TARGET_DIR") {
    command.env("CARGO_TARGET_DIR", target_dir_env);
  }
  let status = command.status()?;
  if !status.success() {
    return Err("cargo build failed".into());
  }

  if let Ok(toolchain) = std::env::var("RUSTUP_TOOLCHAIN") {
    let toolchain_name = format!(
      "{}@{}{}",
      plain_name.trim_end_matches(std::env::consts::DLL_SUFFIX),
      toolchain,
      std::env::consts::DLL_SUFFIX
    );
    let toolchain_path = target_dir.join(toolchain_name);
    fs::create_dir_all(&target_dir)?;
    fs::copy(&plain_path, &toolchain_path)?;
    Ok(toolchain_path)
  } else {
    Ok(plain_path)
  }
}
