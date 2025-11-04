use cellactor_utils_core_rs::sync::ArcShared;

use super::MailboxInstrumentation;
use crate::{NoStdToolbox, actor_prim::Pid, system::SystemState};

#[test]
fn mailbox_instrumentation_new() {
  let system_state = ArcShared::new(SystemState::<NoStdToolbox>::new());
  let pid = Pid::new(1, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(100), Some(50), Some(80));
  // ??????????????
  let _ = instrumentation;
}

#[test]
fn mailbox_instrumentation_clone() {
  let system_state = ArcShared::new(SystemState::<NoStdToolbox>::new());
  let pid = Pid::new(2, 0);
  let instrumentation1 = MailboxInstrumentation::new(system_state.clone(), pid, None, None, None);
  let instrumentation2 = instrumentation1.clone();
  // ????????????
  let _ = instrumentation1;
  let _ = instrumentation2;
}

#[test]
fn mailbox_instrumentation_publish() {
  let system_state = ArcShared::new(SystemState::<NoStdToolbox>::new());
  let pid = Pid::new(3, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(100), Some(50), None);
  // publish???????????????????????????????
  instrumentation.publish(10, 5);
}

#[test]
fn mailbox_instrumentation_publish_with_warning() {
  let system_state = ArcShared::new(SystemState::<NoStdToolbox>::new());
  let pid = Pid::new(4, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(100), Some(50), Some(80));
  // ???????????publish???
  instrumentation.publish(80, 5);
  instrumentation.publish(100, 5);
}
