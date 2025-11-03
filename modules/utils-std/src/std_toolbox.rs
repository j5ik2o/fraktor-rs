use cellactor_utils_core_rs::sync::RuntimeToolbox;

use crate::toolbox::StdMutexFamily;

/// Toolbox for std environments, backed by [`StdMutexFamily`].
#[derive(Clone, Copy, Debug, Default)]
pub struct StdToolbox;

impl RuntimeToolbox for StdToolbox {
  type MutexFamily = StdMutexFamily;
}
