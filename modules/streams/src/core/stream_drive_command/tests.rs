use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::stream_drive_command::StreamDriveCommand;

#[test]
fn tick_command_can_be_constructed() {
  let _command = StreamDriveCommand::<NoStdToolbox>::Tick;
}
