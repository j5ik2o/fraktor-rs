use crate::core::r#impl::materialization::StreamDriveCommand;

#[test]
fn tick_command_can_be_constructed() {
  let _command = StreamDriveCommand::Tick;
}
