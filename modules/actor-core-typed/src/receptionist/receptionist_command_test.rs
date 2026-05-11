use core::any::TypeId;

use crate::receptionist::ReceptionistCommand;

#[test]
fn receptionist_command_should_be_accessible() {
  let _: fn() -> usize = || core::mem::size_of::<ReceptionistCommand>();
  assert_eq!(TypeId::of::<u32>(), TypeId::of::<u32>());
}
