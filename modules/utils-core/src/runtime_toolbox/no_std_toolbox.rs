use super::RuntimeToolbox;
use crate::runtime_toolbox::sync_mutex_family::SpinMutexFamily;

#[cfg(test)]
mod tests;

/// Default toolbox for no_std environments, backed by [`SpinMutexFamily`].
#[derive(Clone, Copy, Debug, Default)]
pub struct NoStdToolbox;

impl RuntimeToolbox for NoStdToolbox {
  type MutexFamily = SpinMutexFamily;
}
