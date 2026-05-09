extern crate std;

use std::{env, fs::OpenOptions};

use super::FileIO;

#[test]
fn to_path_constructs_sink_without_panicking() {
  let mut path = env::temp_dir();
  path.push("fraktor_file_io_to_path_test.bin");
  let _sink = FileIO::to_path(&path);
}

#[test]
fn to_path_with_options_constructs_sink_without_panicking() {
  let mut path = env::temp_dir();
  path.push("fraktor_file_io_to_path_with_options_test.bin");
  let mut options = OpenOptions::new();
  options.create(true).write(true).truncate(true);
  let _sink = FileIO::to_path_with_options(&path, options);
}

#[test]
fn to_path_with_position_constructs_sink_without_panicking() {
  let mut path = env::temp_dir();
  path.push("fraktor_file_io_to_path_with_position_test.bin");
  let mut options = OpenOptions::new();
  options.create(true).write(true);
  let _sink = FileIO::to_path_with_position(&path, options, 0);
}
