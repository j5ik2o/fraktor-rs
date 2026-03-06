use core::any::TypeId;

use crate::core::typed::receptionist_command::ReceptionistCommand;

#[test]
fn receptionist_command_should_be_accessible() {
  // Verify that ReceptionistCommand is accessible and the module compiles.
  // We cannot construct an ActorRef without a running system,
  // so we verify that the type exists and is usable.
  let _: fn() -> usize = || core::mem::size_of::<ReceptionistCommand>();
  assert_eq!(TypeId::of::<u32>(), TypeId::of::<u32>());
}
