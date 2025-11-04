use super::RuntimeToolbox;
use crate::sync::spin_mutex_family::SpinMutexFamily;

/// Default toolbox for no_std environments, backed by [`SpinMutexFamily`].
#[derive(Clone, Copy, Debug, Default)]
pub struct NoStdToolbox;

impl RuntimeToolbox for NoStdToolbox {
  type MutexFamily = SpinMutexFamily;
}
